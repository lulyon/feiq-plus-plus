//! Network lifecycle manager: coordinates UDP + TCP, message parsing, and self-filtering.
//! Equivalent to feiqcommu.cpp + the threading model in feiqengine.cpp.

use crate::network::tcp::{FileServer, FileTransfer};
use crate::network::udp::UdpTransport;
use crate::protocol::constants::{IPMSG_ANSENTRY, IPMSG_BR_ENTRY, IPMSG_BR_EXIT, IPMSG_GETDIRFILES, IPMSG_GETFILEDATA, IPMSG_RELEASEFILES};
use crate::protocol::parser::ProtocolChain;
use crate::protocol::serializer::parse_raw;
use super::NetworkEvent;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;

/// NetworkManager owns the UDP socket and protocol chain,
/// continuously receiving and dispatching packets.
pub struct NetworkManager {
    udp: UdpTransport,
    tcp_server: Arc<FileServer>,
    protocol_chain: ProtocolChain,
    self_mac: String,
    self_name: String,
    event_tx: mpsc::UnboundedSender<NetworkEvent>,
    port: u16,
    shutdown: Arc<AtomicBool>,
}

impl NetworkManager {
    /// Create a new network manager. Binds to the given port.
    pub async fn new(
        event_tx: mpsc::UnboundedSender<NetworkEvent>,
        self_name: String,
        port: u16,
    ) -> anyhow::Result<Self> {
        let udp = UdpTransport::bind(port).await?;
        let tcp_server = Arc::new(FileServer::bind(port).await?);
        let self_mac = udp.mac().to_string();
        let protocol_chain = crate::protocol::parser::build_default_chain();

        tracing::info!(
            "NetworkManager started: mac={self_mac}, name={self_name}"
        );

        Ok(Self {
            udp,
            tcp_server,
            protocol_chain,
            self_mac,
            self_name,
            event_tx,
            port,
            shutdown: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Signal the receive loop to stop
    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::Relaxed);
    }

    /// Get the detected MAC address
    pub fn self_mac(&self) -> &str {
        &self.self_mac
    }

    /// Get the self name
    pub fn self_name(&self) -> &str {
        &self.self_name
    }

    /// Get the port this instance is bound to
    pub fn port(&self) -> u16 { self.port }

    /// Send UDP data to a specific IP:port
    pub async fn send_to(&self, ip: &str, port: u16, data: &[u8]) -> anyhow::Result<()> {
        tracing::debug!("UDP send {} bytes to {ip}:{port}", data.len());
        self.udp.send_to(ip, port, data).await
    }

    /// Broadcast UDP data to the LAN on own port
    pub async fn broadcast(&self, data: &[u8]) -> anyhow::Result<()> {
        self.udp.broadcast(self.port, data).await
    }

    /// Broadcast to a specific port (for cross-port discovery)
    pub async fn broadcast_to_port(&self, port: u16, data: &[u8]) -> anyhow::Result<()> {
        self.udp.broadcast(port, data).await
    }

    /// Connect to a remote peer for file transfer (TCP)
    pub async fn connect_for_file(&self, ip: &str, port: u16) -> anyhow::Result<FileTransfer> {
        FileTransfer::connect(ip, port).await
    }

    /// Accept an incoming file transfer connection
    pub async fn accept_file_transfer(&self) -> anyhow::Result<(FileTransfer, String)> {
        let (stream, ip) = self.tcp_server.accept().await?;
        Ok((FileTransfer::from_stream(stream), ip))
    }

    /// Start a background task that accepts incoming TCP file transfer connections
    /// and queues them for the engine to claim via `accept_queued_transfer()`.
    /// This prevents the kernel TCP backlog from filling up when multiple peers
    /// connect simultaneously, without racing with the engine's accept logic.
    pub fn start_accept_loop(self: &Arc<Self>) -> mpsc::UnboundedReceiver<(FileTransfer, String)> {
        let (queue_tx, queue_rx) = mpsc::unbounded_channel();
        let this = self.clone();
        tokio::spawn(async move {
            loop {
                match this.tcp_server.accept().await {
                    Ok((stream, ip)) => {
                        tracing::debug!("TCP server (background) accepted connection from {ip}");
                        let ft = FileTransfer::from_stream(stream);
                        if queue_tx.send((ft, ip)).is_err() {
                            // Engine dropped receiver, shut down
                            break;
                        }
                    }
                    Err(e) => {
                        tracing::error!("TCP server accept loop error: {e}");
                        break;
                    }
                }
            }
        });
        queue_rx
    }

    /// Main receive loop. Runs until shutdown is signaled.
    pub async fn run(&self) -> anyhow::Result<()> {
        loop {
            if self.shutdown.load(Ordering::Relaxed) {
                tracing::info!("NetworkManager receive loop shutting down");
                break;
            }
            match self.udp.recv_from().await {
                Ok((data, ip, port)) => {
                    tracing::trace!("UDP recv {} bytes from {ip}:{port}", data.len());
                    self.handle_packet(&data, &ip, port);
                }
                Err(e) => {
                    tracing::error!("UDP recv error: {e}");
                    let _ = self.event_tx.send(NetworkEvent::Error(e.to_string()));
                }
            }
        }
        Ok(())
    }

    /// Parse and dispatch a single received packet
    fn handle_packet(&self, data: &[u8], ip: &str, sender_port: u16) {
        // Parse raw data through serializer (includes self-filter)
        let mut post = match parse_raw(data, ip, sender_port, &self.self_mac, &self.self_name) {
            Some(post) => post,
            None => return, // filtered or malformed
        };

        // Run through protocol chain to parse contents
        self.protocol_chain.process(&mut post);

        // ─── Check for GETFILEDATA (file data request) ────────
        if (is_cmd_set(post.cmd_id, IPMSG_GETFILEDATA) || is_cmd_set(post.cmd_id, IPMSG_GETDIRFILES))
            && post.get_file_data.is_some()
        {
            let gfd = post.get_file_data.take().unwrap();
            let _ = self.event_tx.send(NetworkEvent::GetFileData {
                packet_no: gfd.packet_no,
                file_id: gfd.file_id,
                offset: gfd.offset,
                from: post.from,
            });
            return;
        }

        // ─── Check for RELEASEFILES ───────────────────────────
        if is_cmd_set(post.cmd_id, IPMSG_RELEASEFILES) && post.get_file_data.is_some() {
            let gfd = post.get_file_data.take().unwrap();
            let _ = self.event_tx.send(NetworkEvent::ReleaseFiles {
                packet_no: gfd.packet_no,
                file_id: gfd.file_id,
                from: post.from,
            });
            return;
        }

        // ─── Dispatch based on parsed contents ────────────────
        if post.contents.is_empty() {
            // System message: online/offline/ansentry
            if is_cmd_set(post.cmd_id, IPMSG_BR_ENTRY) {
                let _ = self.event_tx.send(NetworkEvent::FellowOnline(post));
            } else if is_cmd_set(post.cmd_id, IPMSG_BR_EXIT) {
                let _ = self.event_tx.send(NetworkEvent::FellowOffline(post));
            } else if is_cmd_set(post.cmd_id, IPMSG_ANSENTRY) {
                let _ = self.event_tx.send(NetworkEvent::FellowAnsEntry(post));
            }
            // Other empty-content posts are ignored
        } else {
            let _ = self.event_tx.send(NetworkEvent::Message(post));
        }
    }
}

use crate::protocol::constants::is_cmd_set;

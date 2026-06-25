//! Network lifecycle manager: coordinates UDP + TCP, message parsing, and self-filtering.
//! Equivalent to feiqcommu.cpp + the threading model in feiqengine.cpp.

use crate::network::tcp::{FileServer, FileTransfer};
use crate::network::udp::UdpTransport;
use crate::protocol::constants::IPMSG_PORT;
use crate::protocol::parser::ProtocolChain;
use crate::protocol::serializer::parse_raw;
use crate::protocol::types::Post;
use tokio::sync::mpsc;

/// Events from the network layer to the engine
#[derive(Debug)]
pub enum NetworkEvent {
    /// Raw parsed Post (for content processing)
    Message(Post),
    /// A new user came online (BR_ENTRY handled)
    FellowOnline(Post),
    /// A user went offline (BR_EXIT handled)
    FellowOffline(Post),
    /// Self online notification response (ANSENTRY handled)
    FellowAnsEntry(Post),
    /// Error in network processing
    Error(String),
}

/// NetworkManager owns the UDP socket and protocol chain,
/// continuously receiving and dispatching packets.
pub struct NetworkManager {
    udp: UdpTransport,
    #[allow(dead_code)]
    tcp_server: FileServer,
    protocol_chain: ProtocolChain,
    self_mac: String,
    self_name: String,
    event_tx: mpsc::UnboundedSender<NetworkEvent>,
    port: u16,
}

impl NetworkManager {
    /// Create a new network manager. Binds to the given port.
    pub async fn new(
        event_tx: mpsc::UnboundedSender<NetworkEvent>,
        self_name: String,
        port: u16,
    ) -> anyhow::Result<Self> {
        let udp = UdpTransport::bind(port).await?;
        let tcp_server = FileServer::bind(port).await?;
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
        })
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

    /// Main receive loop. Runs forever, dispatching parsed packets.
    pub async fn run(&mut self) -> anyhow::Result<()> {
        loop {
            match self.udp.recv_from().await {
                Ok((data, ip)) => {
                    self.handle_packet(&data, &ip);
                }
                Err(e) => {
                    tracing::error!("UDP recv error: {e}");
                    let _ = self.event_tx.send(NetworkEvent::Error(e.to_string()));
                }
            }
        }
    }

    /// Parse and dispatch a single received packet
    fn handle_packet(&self, data: &[u8], ip: &str) {
        // Parse raw data through serializer (includes self-filter)
        let mut post = match parse_raw(data, ip, &self.self_mac, &self.self_name) {
            Some(post) => post,
            None => return, // filtered or malformed
        };

        // Run through protocol chain to parse contents
        self.protocol_chain.process(&mut post);

        // Dispatch based on what was parsed
        if post.contents.is_empty() {
            // System message: online/offline/ansentry
            if is_cmd_set(post.cmd_id, crate::protocol::constants::IPMSG_BR_ENTRY) {
                let _ = self.event_tx.send(NetworkEvent::FellowOnline(post));
            } else if is_cmd_set(post.cmd_id, crate::protocol::constants::IPMSG_BR_EXIT) {
                let _ = self.event_tx.send(NetworkEvent::FellowOffline(post));
            } else if is_cmd_set(post.cmd_id, crate::protocol::constants::IPMSG_ANSENTRY) {
                let _ = self.event_tx.send(NetworkEvent::FellowAnsEntry(post));
            }
            // Other empty-content posts are ignored
        } else {
            let _ = self.event_tx.send(NetworkEvent::Message(post));
        }
    }
}

use crate::protocol::constants::is_cmd_set;

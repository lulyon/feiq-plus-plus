//! Relay client transport — WebSocket connection to feiq-relay server.
//! Produces the same NetworkEvent variants as the UDP transport,
//! so the engine doesn't need to know where messages came from.

use super::NetworkEvent;
use crate::protocol::constants::is_cmd_set;
use crate::protocol::parser::ProtocolChain;
use crate::protocol::serializer::{parse_raw, parse_version_info};
use crate::protocol::types::*;
use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::sleep;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

// ─── Relay protocol messages (server → client) ──────────────

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ServerMsg {
    Joined {
        client_id: String,
        peers: Vec<PeerData>,
    },
    PeerOnline {
        peer: PeerData,
    },
    PeerOffline {
        peer_id: String,
    },
    Message {
        from: String,
        from_name: String,
        ipmsg_cmd: u32,
        ipmsg_data: String,
    },
    Broadcast {
        from: String,
        from_name: String,
        ipmsg_cmd: u32,
        ipmsg_data: String,
    },
    OfflineMsgs {
        messages: Vec<OfflineMsgData>,
    },
    Pong,
    Error {
        message: String,
    },
}

#[derive(Debug, Clone, Deserialize)]
struct PeerData {
    id: String,
    name: String,
    host: String,
    version: String,
}

#[derive(Debug, Clone, Deserialize)]
struct OfflineMsgData {
    from: String,
    from_name: String,
    ipmsg_cmd: u32,
    ipmsg_data: String,
    timestamp: i64,
}

// ─── RelayClient ─────────────────────────────────────────────

/// WebSocket-based relay client that speaks the feiq-relay JSON protocol.
pub struct RelayClient {
    url: String,
    room: String,
    self_name: String,
    self_host: String,
    self_version: String,
    client_id: Arc<std::sync::Mutex<Option<String>>>,
    /// Maps relay peer_id → local IP-like key ("relay:{peer_id}")
    peer_map: Arc<std::sync::Mutex<HashMap<String, String>>>,
    event_tx: mpsc::UnboundedSender<NetworkEvent>,
    shutdown: Arc<AtomicBool>,
    protocol_chain: ProtocolChain,
    /// Shared write channel to the active WebSocket's write task.
    /// Set when connected, cleared on disconnect. Used by send_to/broadcast
    /// to reuse the persistent connection instead of opening a new one per message.
    ws_write_tx: Arc<std::sync::Mutex<Option<mpsc::UnboundedSender<String>>>>,
}

impl RelayClient {
    /// Create but do NOT connect. Call `run()` to start the receive loop.
    pub fn new(
        url: &str,
        room: &str,
        name: &str,
        host: &str,
        version: &str,
        event_tx: mpsc::UnboundedSender<NetworkEvent>,
    ) -> Self {
        Self {
            url: url.to_string(),
            room: room.to_string(),
            self_name: name.to_string(),
            self_host: host.to_string(),
            self_version: version.to_string(),
            client_id: Arc::new(std::sync::Mutex::new(None)),
            peer_map: Arc::new(std::sync::Mutex::new(HashMap::new())),
            event_tx,
            shutdown: Arc::new(AtomicBool::new(false)),
            protocol_chain: crate::protocol::parser::build_default_chain(),
            ws_write_tx: Arc::new(std::sync::Mutex::new(None)),
        }
    }

    /// Signal shutdown — the receive loop will exit on next iteration.
    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::Relaxed);
    }

    /// Per-peer IP key used in ContactBook.
    fn peer_ip(peer_id: &str) -> String {
        format!("relay:{peer_id}")
    }

    // ─── Peer mapping helpers ─────────────────────────────────

    fn register_peer(&self, peer_id: &str) -> String {
        let ip = Self::peer_ip(peer_id);
        self.peer_map
            .lock()
            .unwrap()
            .insert(peer_id.to_string(), ip.clone());
        ip
    }

    fn lookup_peer_id(&self, ip: &str) -> Option<String> {
        self.peer_map
            .lock()
            .unwrap()
            .iter()
            .find(|(_, v)| *v == ip)
            .map(|(k, _)| k.clone())
    }

    // ─── Public send API ──────────────────────────────────────

    /// Send an IPMSG datagram to a specific peer through the relay.
    pub async fn send_to(&self, peer_id: &str, cmd: u32, data: &[u8]) -> anyhow::Result<()> {
        let msg = serde_json::json!({
            "type": "send",
            "to": peer_id,
            "ipmsg_cmd": cmd,
            "ipmsg_data": base64::engine::general_purpose::STANDARD.encode(data),
        });
        // We need a connection — the run() task owns the WS.
        // For now, this is called from engine context which holds a WS sender.
        // We'll connect inline if needed.
        self.send_json(&msg).await
    }

    /// Broadcast an IPMSG datagram to all peers in the room.
    pub async fn broadcast(&self, cmd: u32, data: &[u8]) -> anyhow::Result<()> {
        let msg = serde_json::json!({
            "type": "broadcast",
            "ipmsg_cmd": cmd,
            "ipmsg_data": base64::engine::general_purpose::STANDARD.encode(data),
        });
        self.send_json(&msg).await
    }

    // ─── Internal — connection + send ─────────────────────────

    /// Connect, join, and start the receive loop. Blocks until shutdown.
    pub async fn run(&self) -> anyhow::Result<()> {
        let mut backoff = 1u64;

        loop {
            if self.shutdown.load(Ordering::Relaxed) {
                break;
            }

            match self.connect_and_serve().await {
                Ok(()) => {
                    // Clean disconnect — reset backoff
                    backoff = 1;
                }
                Err(e) => {
                    tracing::warn!("Relay disconnected: {e}. Reconnecting in {backoff}s...");
                    sleep(Duration::from_secs(backoff)).await;
                    backoff = std::cmp::min(backoff * 2, 60);
                }
            }

            if self.shutdown.load(Ordering::Relaxed) {
                break;
            }
        }
        tracing::info!("RelayClient run loop exited");
        Ok(())
    }

    async fn connect_and_serve(&self) -> anyhow::Result<()> {
        let url = &self.url;
        tracing::info!("Connecting to relay at {url}");

        let (ws_stream, _) = connect_async(url).await?;
        tracing::info!("WebSocket connected to {url}");

        let (mut ws_tx, mut ws_rx) = ws_stream.split();

        // Send join
        let join_msg = serde_json::json!({
            "type": "join",
            "room": self.room,
            "name": self.self_name,
            "host": self.self_host,
            "version": self.self_version,
        });
        ws_tx
            .send(Message::Text(join_msg.to_string()))
            .await?;

        // Channel for outbound messages from send_to/broadcast (engine side)
        // to the write task (WS side). This avoids opening a new WS connection
        // per message.
        let (tx, mut rx) = mpsc::unbounded_channel::<String>();
        let send_tx = tx.clone();
        // Share with send_json() so engine-triggered send_to/broadcast reuse
        // the persistent WS connection instead of opening a new one.
        *self.ws_write_tx.lock().unwrap() = Some(tx);

        let writer_handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    msg = rx.recv() => {
                        match msg {
                            Some(text) => {
                                if ws_tx.send(Message::Text(text.into())).await.is_err() {
                                    break;
                                }
                            }
                            None => break,
                        }
                    }
                }
            }
        });

        // Main receive loop
        loop {
            tokio::select! {
                ws_msg = ws_rx.next() => {
                    match ws_msg {
                        Some(Ok(Message::Text(text))) => {
                            self.handle_message(&text);
                        }
                        Some(Ok(Message::Close(_))) | None => {
                            tracing::info!("Relay WS closed");
                            break;
                        }
                        Some(Ok(Message::Ping(data))) => {
                            let _ = send_tx.send(serde_json::to_string(&serde_json::json!({
                                "type": "pong"
                            })).unwrap());
                            let _ = data; // silently ignore ping payload
                        }
                        Some(Ok(_)) => {} // ignore binary
                        Some(Err(e)) => {
                            tracing::error!("Relay WS error: {e}");
                            break;
                        }
                    }
                }
                _ = sleep(Duration::from_secs(30)) => {
                    // Send heartbeat ping
                    let _ = send_tx.send(serde_json::to_string(&serde_json::json!({
                        "type": "ping"
                    })).unwrap());
                }
            }
        }

        // Clear shared write channel on disconnect so send_json returns error
        // instead of queuing messages that will never be sent.
        *self.ws_write_tx.lock().unwrap() = None;
        writer_handle.abort();
        Ok(())
    }

    fn handle_message(&self, text: &str) {
        let msg: ServerMsg = match serde_json::from_str(text) {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!("Failed to parse relay message: {e}");
                return;
            }
        };

        match msg {
            ServerMsg::Joined { client_id, peers } => {
                *self.client_id.lock().unwrap() = Some(client_id);
                for peer in &peers {
                    self.register_and_emit_online(peer);
                }
            }
            ServerMsg::PeerOnline { peer } => {
                self.register_and_emit_online(&peer);
            }
            ServerMsg::PeerOffline { peer_id } => {
                if let Some(ip) = self.peer_map.lock().unwrap().remove(&peer_id) {
                    let mut post = Post::new(&ip);
                    post.cmd_id = crate::protocol::constants::IPMSG_BR_EXIT;
                    post.from.online = false;
                    // Fill in what we know
                    let _ = self.event_tx.send(NetworkEvent::FellowOffline(post));
                }
            }
            ServerMsg::Message {
                from,
                from_name,
                ipmsg_cmd,
                ipmsg_data,
            } => {
                self.dispatch_ipmsg(&from, &from_name, ipmsg_cmd, &ipmsg_data, None);
            }
            ServerMsg::Broadcast {
                from,
                from_name,
                ipmsg_cmd,
                ipmsg_data,
            } => {
                self.dispatch_ipmsg(&from, &from_name, ipmsg_cmd, &ipmsg_data, None);
            }
            ServerMsg::OfflineMsgs { messages } => {
                for m in &messages {
                    self.dispatch_ipmsg(
                        &m.from,
                        &m.from_name,
                        m.ipmsg_cmd,
                        &m.ipmsg_data,
                        Some(m.timestamp),
                    );
                }
            }
            ServerMsg::Pong => {
                tracing::trace!("Relay pong");
            }
            ServerMsg::Error { message } => {
                let _ = self.event_tx.send(NetworkEvent::Error(message));
            }
        }
    }

    fn register_and_emit_online(&self, peer: &PeerData) {
        let ip = self.register_peer(&peer.id);
        let version_info = parse_version_info(&peer.version);

        let mut post = Post::new(&ip);
        post.cmd_id = crate::protocol::constants::IPMSG_BR_ENTRY;
        post.from.name = peer.name.clone();
        post.from.host = peer.host.clone();
        post.from.version = peer.version.clone();
        post.from.mac = version_info.mac;
        post.from.online = true;
        post.from.source = PeerSource::RelayPeer(peer.id.clone());
        post.from.pc_name = peer.name.clone();

        let _ = self.event_tx.send(NetworkEvent::FellowOnline(post));
    }

    fn dispatch_ipmsg(
        &self,
        from_id: &str,
        from_name: &str,
        cmd: u32,
        ipmsg_data_b64: &str,
        override_timestamp: Option<i64>,
    ) {
        let data = match base64::engine::general_purpose::STANDARD.decode(ipmsg_data_b64) {
            Ok(d) => d,
            Err(e) => {
                tracing::warn!("Failed to decode ipmsg_data: {e}");
                return;
            }
        };

        let ip = Self::peer_ip(from_id);
        let mut post = match parse_raw(&data, &ip, 0, "", "") {
            Some(p) => p,
            None => {
                // parse_raw might filter due to self-matching; for relay, we bypass.
                // Build a minimal post manually.
                let mut p = Post::new(&ip);
                p.cmd_id = cmd;
                p.extra = data.clone();
                p
            }
        };

        post.from.name = from_name.to_string();
        post.from.source = PeerSource::RelayPeer(from_id.to_string());
        // Note: post.cmd_id is already set correctly:
        // - from parse_raw() if it succeeded (includes option flags from wire binary)
        // - from the None branch above if parse_raw() failed (set to relay's `cmd`)
        // Do NOT overwrite — relay JSON's ipmsg_cmd may lack flags like IPMSG_ENCRYPTOPT

        if let Some(ts) = override_timestamp {
            // Timestamp from offline message
            post.when = chrono::DateTime::from_timestamp_millis(ts)
                .unwrap_or(post.when);
        }

        // Run through protocol chain to parse contents
        self.protocol_chain.process(&mut post);

        // Dispatch
        if post.contents.is_empty() {
            // System messages (online/offline handled by peer_online/peer_offline)
            if is_cmd_set(cmd, crate::protocol::constants::IPMSG_BR_EXIT) {
                let _ = self.event_tx.send(NetworkEvent::FellowOffline(post));
            }
        } else {
            let _ = self.event_tx.send(NetworkEvent::Message(post));
        }
    }

    async fn send_json(&self, msg: &serde_json::Value) -> anyhow::Result<()> {
        // Use the persistent WebSocket connection's write channel instead of
        // opening a new connection per message (which would require a new Join
        // handshake and be silently dropped by the server).
        let tx_guard = self.ws_write_tx.lock().unwrap();
        match tx_guard.as_ref() {
            Some(tx) => {
                tx.send(msg.to_string())
                    .map_err(|e| anyhow::anyhow!("Relay send channel closed: {e}"))?;
                Ok(())
            }
            None => Err(anyhow::anyhow!("Relay not connected — call run() first")),
        }
    }
}

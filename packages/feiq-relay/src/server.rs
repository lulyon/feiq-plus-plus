//! WebSocket relay server — room management, message routing, offline queue.

use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::{mpsc, Mutex};
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;
use uuid::Uuid;

// ─── Relay protocol messages ────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ClientMessage {
    Join {
        room: String,
        name: String,
        host: String,
        version: String,
    },
    Leave,
    Send {
        to: String,
        ipmsg_cmd: u32,
        ipmsg_data: String,
    },
    Broadcast {
        ipmsg_cmd: u32,
        ipmsg_data: String,
    },
    Ping,
    /// Initiate a binary file transfer stream through the relay
    FileStart {
        to: String,
        file_id: u64,
        file_name: String,
        file_size: u64,
    },
    /// End a binary file transfer stream
    FileEnd {
        to: String,
        file_id: u64,
    },
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ServerMessage<'a> {
    Joined {
        client_id: String,
        peers: Vec<PeerInfo>,
    },
    PeerOnline {
        peer: PeerInfo,
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
        messages: Vec<&'a PendingMessage>,
    },
    Pong,
    /// Forward file transfer start notification to the receiver
    FileStart {
        from: String,
        from_name: String,
        file_id: u64,
        file_name: String,
        file_size: u64,
    },
    /// Forward file transfer end notification to the receiver
    FileEnd {
        from: String,
        file_id: u64,
    },
    Error {
        message: String,
    },
}

#[derive(Debug, Clone, Serialize)]
struct PeerInfo {
    id: String,
    name: String,
    host: String,
    version: String,
}

// ─── Room state ─────────────────────────────────────────────

struct ClientState {
    id: String,
    name: String,
    host: String,
    version: String,
    /// Stable identity for offline message routing (survives reconnect with new UUID)
    peer_key: String,
    tx: mpsc::UnboundedSender<Message>,
}

/// Max offline messages per peer to prevent memory DoS
const MAX_OFFLINE_PER_PEER: usize = 200;

#[derive(Debug, Clone, Serialize)]
struct PendingMessage {
    /// Stable peer key (name@host), survives client reconnect with new UUID.
    /// Serialized as "to" for backward compatibility with relay clients.
    #[serde(rename = "to")]
    to_peer_key: String,
    /// Serialized as "from" for backward compatibility.
    #[serde(rename = "from")]
    from_id: String,
    from_name: String,
    ipmsg_cmd: u32,
    ipmsg_data: String,
    timestamp: i64,
}

/// Tracks an active binary file transfer stream between two peers
struct FileTransferStream {
    receiver_id: String,
}

struct Room {
    clients: HashMap<String, ClientState>,
    offline_queue: Vec<PendingMessage>,
    /// Persistent mapping of client_id -> peer_key that survives disconnection.
    /// Populated on every Join, never cleared. Used to resolve the target's
    /// stable peer_key when queuing offline messages for a disconnected peer.
    peer_key_map: HashMap<String, String>,
    /// Active file transfer streams: (sender_id, file_id) → FileTransferStream
    file_transfers: HashMap<(String, u64), FileTransferStream>,
}

struct Server {
    rooms: HashMap<String, Room>,
}

impl Server {
    fn new() -> Self {
        Self {
            rooms: HashMap::new(),
        }
    }

    fn get_or_create_room(&mut self, name: &str) -> &mut Room {
        self.rooms.entry(name.to_string()).or_insert(Room {
            clients: HashMap::new(),
            offline_queue: Vec::new(),
            peer_key_map: HashMap::new(),
            file_transfers: HashMap::new(),
        })
    }
}

// ─── Main entry ─────────────────────────────────────────────

pub async fn run(bind: &str, port: u16, history_ttl: u64) -> anyhow::Result<()> {
    let server = Arc::new(Mutex::new(Server::new()));
    let listener = TcpListener::bind(format!("{bind}:{port}")).await?;
    tracing::info!("Listening on ws://{bind}:{port}");

    // Spawn TTL cleanup task
    let server_c = server.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
            let cutoff = chrono_now() - (history_ttl as i64 * 1000);
            let mut s = server_c.lock().await;
            for room in s.rooms.values_mut() {
                room.offline_queue.retain(|m| m.timestamp > cutoff);
            }
        }
    });

    loop {
        let (stream, addr) = listener.accept().await?;
        tracing::info!("New connection from {addr}");

        let server = server.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream, server, addr).await {
                tracing::error!("Connection {addr} error: {e}");
            }
        });
    }
}

fn chrono_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

// ─── Per-connection handler ─────────────────────────────────

async fn handle_connection(
    stream: tokio::net::TcpStream,
    server: Arc<Mutex<Server>>,
    addr: std::net::SocketAddr,
) -> anyhow::Result<()> {
    let ws_stream = accept_async(stream).await?;
    let (mut ws_tx, mut ws_rx) = ws_stream.split();

    // Channel for async writes from server broadcast
    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();

    let mut client_id: Option<String> = None;
    let mut room_name: Option<String> = None;

    // Spawn write-forwarding task
    let write_handle = tokio::spawn(async move {
        loop {
            tokio::select! {
                msg = rx.recv() => {
                    match msg {
                        Some(m) => {
                            if ws_tx.send(m).await.is_err() {
                                break;
                            }
                        }
                        None => break,
                    }
                }
            }
        }
    });

    // Read loop
    loop {
        tokio::select! {
            ws_msg = ws_rx.next() => {
                match ws_msg {
                    Some(Ok(Message::Text(text))) => {
                        let msg: ClientMessage = match serde_json::from_str(&text) {
                            Ok(m) => m,
                            Err(e) => {
                                let _ = tx.send(Message::Text(
                                    serde_json::to_string(&ServerMessage::Error {
                                        message: format!("invalid json: {e}"),
                                    }).unwrap()
                                ));
                                continue;
                            }
                        };
                        handle_client_msg(msg, &tx, &server, &mut client_id, &mut room_name).await;
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        // Client disconnected
                        break;
                    }
                    Some(Ok(Message::Binary(data))) => {
                        // Binary file chunk: [8 bytes file_id BE][chunk data]
                        if data.len() >= 8 {
                            let file_id = u64::from_be_bytes([
                                data[0], data[1], data[2], data[3],
                                data[4], data[5], data[6], data[7],
                            ]);
                            let s = server.lock().await;
                            if let (Some(rid), Some(cid)) = (&room_name, &client_id) {
                                if let Some(room) = s.rooms.get(rid) {
                                    if let Some(stream) = room.file_transfers.get(&(cid.clone(), file_id)) {
                                        if let Some(target) = room.clients.get(&stream.receiver_id) {
                                            // Forward binary chunk (with file_id prefix) to receiver
                                            let _ = target.tx.send(Message::Binary(data));
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Some(Ok(Message::Ping(_)))
                    | Some(Ok(Message::Pong(_)))
                    | Some(Ok(Message::Frame(_))) => {} // ignore ping/pong/frame
                    Some(Err(e)) => {
                        tracing::warn!("WS error from {addr}: {e}");
                        break;
                    }
                }
            }
        }
    }

    // Cleanup on disconnect
    if let (Some(rid), Some(cid)) = (&room_name, &client_id) {
        let mut s = server.lock().await;
        if let Some(room) = s.rooms.get_mut(rid) {
            let client_peer_key = room.clients.get(cid).map(|c| c.peer_key.clone());
            let client_name = room.clients.get(cid).map(|c| c.name.clone()).unwrap_or_default();

            // Clean up file transfer streams owned by this client
            room.file_transfers.retain(|(sender_id, _), _| sender_id != cid);
            room.clients.remove(cid);

            // Clean up orphaned peer_key_map entry: remove this UUID from
            // the mapping only if no other connected client shares the same
            // peer_key (prevents removing the mapping for a reconnected peer).
            if let Some(ref pk) = client_peer_key {
                let other_with_same_key = room.clients.values().any(|c| c.peer_key == *pk);
                if !other_with_same_key {
                    room.peer_key_map.remove(cid);
                }
            }

            let offline_msg = serde_json::to_string(&ServerMessage::PeerOffline {
                peer_id: cid.clone(),
            })
            .unwrap();
            for peer in room.clients.values() {
                let _ = peer.tx.send(Message::Text(offline_msg.clone()));
            }
            tracing::info!("Client {cid} ({client_name}) left room {rid}");
        }
    }

    write_handle.abort();
    tracing::info!("Connection {addr} closed");
    Ok(())
}

async fn handle_client_msg(
    msg: ClientMessage,
    tx: &mpsc::UnboundedSender<Message>,
    server: &Arc<Mutex<Server>>,
    client_id: &mut Option<String>,
    room_name: &mut Option<String>,
) {
    match msg {
        ClientMessage::Join {
            room,
            name,
            host,
            version,
        } => {
            let mut s = server.lock().await;
            let room_obj = s.get_or_create_room(&room);
            let id = Uuid::new_v4().to_string();
            // Stable identity for offline message routing (survives reconnect)
            let peer_key = format!("{name}@{host}");

            // If an old session for this peer exists (same name@host, different UUID),
            // remove it before inserting the new one (handles reconnect on new WS)
            let old_id = room_obj
                .clients
                .iter()
                .find(|(_, c)| c.peer_key == peer_key)
                .map(|(k, _)| k.clone());
            if let Some(ref old) = old_id {
                room_obj.clients.remove(old);
                // Clean up orphaned peer_key_map entry for the stale session
                room_obj.peer_key_map.remove(old);
                tracing::info!("Removed stale session {old} for {peer_key}");
            }

            // Get peers BEFORE inserting self
            let peers: Vec<PeerInfo> = room_obj
                .clients
                .values()
                .map(|c| PeerInfo {
                    id: c.id.clone(),
                    name: c.name.clone(),
                    host: c.host.clone(),
                    version: c.version.clone(),
                })
                .collect();

            // Notify existing peers about newcomer
            let online_msg = serde_json::to_string(&ServerMessage::PeerOnline {
                peer: PeerInfo {
                    id: id.clone(),
                    name: name.clone(),
                    host: host.clone(),
                    version: version.clone(),
                },
            })
            .unwrap();
            for peer in room_obj.clients.values() {
                let _ = peer.tx.send(Message::Text(online_msg.clone()));
            }

            // Insert new client
            room_obj.clients.insert(
                id.clone(),
                ClientState {
                    id: id.clone(),
                    name: name.clone(),
                    host: host.clone(),
                    version,
                    peer_key: peer_key.clone(),
                    tx: tx.clone(),
                },
            );

            // Persist UUID -> peer_key mapping for offline message routing.
            // This map survives client disconnection so offline messages can
            // be correctly keyed by stable peer_key even when the target is offline.
            room_obj.peer_key_map.insert(id.clone(), peer_key.clone());

            // Send joined confirmation with peer list
            let joined_msg = serde_json::to_string(&ServerMessage::Joined {
                client_id: id.clone(),
                peers,
            })
            .unwrap();
            let _ = tx.send(Message::Text(joined_msg));

            // Deliver offline messages keyed by stable peer_key (not ephemeral UUID)
            let offline: Vec<&PendingMessage> = room_obj
                .offline_queue
                .iter()
                .filter(|m| m.to_peer_key == peer_key)
                .collect();
            if !offline.is_empty() {
                let offline_msg = serde_json::to_string(&ServerMessage::OfflineMsgs {
                    messages: offline.clone(),
                })
                .unwrap();
                let _ = tx.send(Message::Text(offline_msg));
                // Remove delivered messages
                room_obj.offline_queue.retain(|m| m.to_peer_key != peer_key);
            }

            *client_id = Some(id);
            *room_name = Some(room);
            tracing::info!("{name} ({peer_key}) joined room {}", room_name.as_ref().unwrap());
        }

        ClientMessage::Leave => {
            // Perform immediate leave: remove self from room and notify peers.
            // This handles explicit leave, while disconnect cleanup handles WS close.
            if let (Some(rid), Some(cid)) = (room_name.as_ref(), client_id.as_ref()) {
                let mut s = server.lock().await;
                if let Some(room) = s.rooms.get_mut(rid) {
                    // Clean up file transfer streams owned by this client
                    room.file_transfers.retain(|(sender_id, _), _| sender_id != cid);
                    if let Some(client) = room.clients.remove(cid) {
                        let peer_key = client.peer_key.clone();
                        // Clean orphaned peer_key_map entry
                        let other_same = room.clients.values().any(|c| c.peer_key == peer_key);
                        if !other_same {
                            room.peer_key_map.remove(cid);
                        }

                        let offline_msg = serde_json::to_string(&ServerMessage::PeerOffline {
                            peer_id: cid.clone(),
                        })
                        .unwrap();
                        for peer in room.clients.values() {
                            let _ = peer.tx.send(Message::Text(offline_msg.clone()));
                        }
                        tracing::info!("{} ({peer_key}) left room {rid}", client.name);
                    }
                }
            }
            *client_id = None;
            *room_name = None;
        }

        ClientMessage::Send {
            to,
            ipmsg_cmd,
            ipmsg_data,
        } => {
            let from_id = client_id.clone().unwrap_or_default();
            let mut s = server.lock().await;
            let room = room_name.as_ref().map(|r| s.rooms.get_mut(r)).flatten();

            if let Some(room) = room {
                let from_name = room
                    .clients
                    .get(&from_id)
                    .map(|c| c.name.clone())
                    .unwrap_or_default();

                if let Some(target) = room.clients.get(&to) {
                    // Online: forward immediately. Keep `from_id` as UUID for
                    // relay client's peer_map compatibility (the client maps
                    // UUID -> synthetic IP in register_peer).
                    let msg = serde_json::to_string(&ServerMessage::Message {
                        from: from_id,
                        from_name,
                        ipmsg_cmd,
                        ipmsg_data,
                    })
                    .unwrap();
                    let _ = target.tx.send(Message::Text(msg));
                } else {
                    // Offline: queue by stable peer_key.
                    // Look up the target's peer_key from the persistent peer_key_map
                    // (which survives client disconnection, unlike room.clients).
                    let target_key = room
                        .peer_key_map
                        .get(&to)
                        .cloned()
                        .unwrap_or_else(|| {
                            // Fallback: use the UUID as-is (for peers never seen before
                            // or if the mapping was somehow lost).
                            tracing::warn!("No peer_key mapping for {to}, falling back to UUID");
                            to.clone()
                        });
                    tracing::debug!("Queued offline message for {target_key}");
                    // Limit queue size per peer to prevent DoS
                    let count_for_target = room
                        .offline_queue
                        .iter()
                        .filter(|m| m.to_peer_key == target_key)
                        .count();
                    if count_for_target < MAX_OFFLINE_PER_PEER {
                        room.offline_queue.push(PendingMessage {
                            to_peer_key: target_key,
                            from_id: from_id,
                            from_name,
                            ipmsg_cmd,
                            ipmsg_data,
                            timestamp: chrono_now(),
                        });
                    } else {
                        tracing::warn!(
                            "Offline queue full for {target_key}, dropping message"
                        );
                    }
                }
            }
        }

        ClientMessage::Broadcast {
            ipmsg_cmd,
            ipmsg_data,
        } => {
            let from_id = client_id.clone().unwrap_or_default();
            let s = server.lock().await;
            if let Some(room) = room_name.as_ref().and_then(|r| s.rooms.get(r)) {
                let from_name = room
                    .clients
                    .get(&from_id)
                    .map(|c| c.name.clone())
                    .unwrap_or_default();

                let msg = serde_json::to_string(&ServerMessage::Broadcast {
                    from: from_id.clone(),
                    from_name,
                    ipmsg_cmd,
                    ipmsg_data: ipmsg_data.clone(),
                })
                .unwrap();
                for (id, peer) in &room.clients {
                    if *id != from_id {
                        let _ = peer.tx.send(Message::Text(msg.clone()));
                    }
                }
            }
        }

        ClientMessage::Ping => {
            let pong = serde_json::to_string(&ServerMessage::Pong).unwrap();
            let _ = tx.send(Message::Text(pong));
        }

        ClientMessage::FileStart {
            to,
            file_id,
            file_name,
            file_size,
        } => {
            let from_id = client_id.clone().unwrap_or_default();
            let mut s = server.lock().await;
            if let Some(room) = room_name.as_ref().and_then(|r| s.rooms.get_mut(r)) {
                let from_name = room
                    .clients
                    .get(&from_id)
                    .map(|c| c.name.clone())
                    .unwrap_or_default();

                // Register the file transfer stream
                room.file_transfers.insert(
                    (from_id.clone(), file_id),
                    FileTransferStream {
                        receiver_id: to.clone(),
                    },
                );

                // Forward FileStart to receiver if online
                if let Some(target) = room.clients.get(&to) {
                    let msg = serde_json::to_string(&ServerMessage::FileStart {
                        from: from_id,
                        from_name,
                        file_id,
                        file_name,
                        file_size,
                    })
                    .unwrap();
                    let _ = target.tx.send(Message::Text(msg));
                }
            }
        }

        ClientMessage::FileEnd { to, file_id } => {
            let from_id = client_id.clone().unwrap_or_default();
            let mut s = server.lock().await;
            if let Some(room) = room_name.as_ref().and_then(|r| s.rooms.get_mut(r)) {
                // Remove the file transfer stream
                room.file_transfers.remove(&(from_id.clone(), file_id));

                // Forward FileEnd to receiver if online
                if let Some(target) = room.clients.get(&to) {
                    let msg = serde_json::to_string(&ServerMessage::FileEnd {
                        from: from_id,
                        file_id,
                    })
                    .unwrap();
                    let _ = target.tx.send(Message::Text(msg));
                }
            }
        }
    }
}

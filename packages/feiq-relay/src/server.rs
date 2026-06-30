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
    tx: mpsc::UnboundedSender<Message>,
}

#[derive(Debug, Clone, Serialize)]
struct PendingMessage {
    to: String,
    from: String,
    from_name: String,
    ipmsg_cmd: u32,
    ipmsg_data: String,
    timestamp: i64,
}

struct Room {
    clients: HashMap<String, ClientState>,
    offline_queue: Vec<PendingMessage>,
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
                    Some(Ok(_)) => {} // ignore binary/ping/pong
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
            let client_name = room.clients.get(cid).map(|c| c.name.clone()).unwrap_or_default();
            room.clients.remove(cid);
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
                    host,
                    version,
                    tx: tx.clone(),
                },
            );

            // Send joined confirmation with peer list
            let joined_msg = serde_json::to_string(&ServerMessage::Joined {
                client_id: id.clone(),
                peers,
            })
            .unwrap();
            let _ = tx.send(Message::Text(joined_msg));

            // Deliver offline messages
            let offline: Vec<&PendingMessage> = room_obj
                .offline_queue
                .iter()
                .filter(|m| m.to == id)
                .collect();
            if !offline.is_empty() {
                let offline_msg = serde_json::to_string(&ServerMessage::OfflineMsgs {
                    messages: offline.clone(),
                })
                .unwrap();
                let _ = tx.send(Message::Text(offline_msg));
                // Remove delivered messages
                room_obj.offline_queue.retain(|m| m.to != id);
            }

            *client_id = Some(id);
            *room_name = Some(room);
            tracing::info!("{name} joined room {}", room_name.as_ref().unwrap());
        }

        ClientMessage::Leave => {
            // Handled by disconnect cleanup
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
                    // Online: forward immediately
                    let msg = serde_json::to_string(&ServerMessage::Message {
                        from: from_id,
                        from_name,
                        ipmsg_cmd,
                        ipmsg_data,
                    })
                    .unwrap();
                    let _ = target.tx.send(Message::Text(msg));
                } else {
                    // Offline: queue
                    tracing::debug!("Queued offline message for {to}");
                    room.offline_queue.push(PendingMessage {
                        to,
                        from: from_id,
                        from_name,
                        ipmsg_cmd,
                        ipmsg_data,
                        timestamp: chrono_now(),
                    });
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
    }
}

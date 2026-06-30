//! Integration test: relay server + client message exchange.

use feiq_relay::server;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use futures_util::{SinkExt, StreamExt};
use std::time::Duration;

const PORT: u16 = 14226;

#[tokio::test]
async fn test_join_and_ping() {
    let _ = tokio::spawn(server::run("127.0.0.1", PORT, 3600));
    tokio::time::sleep(Duration::from_millis(200)).await;

    let url = format!("ws://127.0.0.1:{PORT}");
    let (mut ws, _) = connect_async(&url).await.unwrap();

    ws.send(Message::Text(serde_json::json!({
        "type": "join", "room": "test", "name": "Alice", "host": "mbp", "version": "v1"
    }).to_string())).await.unwrap();

    let text = ws.next().await.unwrap().unwrap().into_text().unwrap();
    let resp: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(resp["type"], "joined");
    assert!(!resp["client_id"].as_str().unwrap().is_empty());
    assert!(resp["peers"].as_array().unwrap().is_empty());

    // Ping/Pong
    ws.send(Message::Text(serde_json::json!({"type":"ping"}).to_string())).await.unwrap();
    let text = ws.next().await.unwrap().unwrap().into_text().unwrap();
    assert_eq!(serde_json::from_str::<serde_json::Value>(&text).unwrap()["type"], "pong");

    ws.close(None).await.unwrap();
}

#[tokio::test]
async fn test_two_clients_discovery_and_message() {
    let _ = tokio::spawn(server::run("127.0.0.1", PORT + 1, 3600));
    tokio::time::sleep(Duration::from_millis(200)).await;

    let url = format!("ws://127.0.0.1:{}", PORT + 1);

    // Alice joins
    let (mut ws_a, _) = connect_async(&url).await.unwrap();
    ws_a.send(Message::Text(serde_json::json!({
        "type": "join", "room": "test", "name": "Alice", "host": "mbp", "version": "v1"
    }).to_string())).await.unwrap();
    let resp: serde_json::Value = serde_json::from_str(
        &ws_a.next().await.unwrap().unwrap().into_text().unwrap()
    ).unwrap();
    assert_eq!(resp["type"], "joined");

    // Bob joins — should see Alice in peers
    let (mut ws_b, _) = connect_async(&url).await.unwrap();
    ws_b.send(Message::Text(serde_json::json!({
        "type": "join", "room": "test", "name": "Bob", "host": "imac", "version": "v1"
    }).to_string())).await.unwrap();
    let resp: serde_json::Value = serde_json::from_str(
        &ws_b.next().await.unwrap().unwrap().into_text().unwrap()
    ).unwrap();
    assert_eq!(resp["type"], "joined");
    let bob_id = resp["client_id"].as_str().unwrap().to_string();
    let peers = resp["peers"].as_array().unwrap();
    assert_eq!(peers.len(), 1);
    assert_eq!(peers[0]["name"], "Alice");

    // Alice gets peer_online for Bob
    let resp: serde_json::Value = serde_json::from_str(
        &ws_a.next().await.unwrap().unwrap().into_text().unwrap()
    ).unwrap();
    assert_eq!(resp["type"], "peer_online");
    assert_eq!(resp["peer"]["name"], "Bob");

    // Alice sends message to Bob
    ws_a.send(Message::Text(serde_json::json!({
        "type": "send", "to": &bob_id, "ipmsg_cmd": 32,
        "ipmsg_data": "SGVsbG8gQm9i"
    }).to_string())).await.unwrap();

    let resp: serde_json::Value = serde_json::from_str(
        &ws_b.next().await.unwrap().unwrap().into_text().unwrap()
    ).unwrap();
    assert_eq!(resp["type"], "message");
    assert_eq!(resp["from_name"], "Alice");
    assert_eq!(resp["ipmsg_cmd"], 32);
    assert_eq!(resp["ipmsg_data"], "SGVsbG8gQm9i");

    // Broadcast
    ws_a.send(Message::Text(serde_json::json!({
        "type": "broadcast", "ipmsg_cmd": 1, "ipmsg_data": "QlJfRU5UUlk="
    }).to_string())).await.unwrap();

    let resp: serde_json::Value = serde_json::from_str(
        &ws_b.next().await.unwrap().unwrap().into_text().unwrap()
    ).unwrap();
    assert_eq!(resp["type"], "broadcast");
    assert_eq!(resp["from_name"], "Alice");

    ws_a.close(None).await.unwrap();
    ws_b.close(None).await.unwrap();
}

// Note: full offline queue recovery after WS disconnect requires client identity
// persistence. The current server queues messages by client_id, and a reconnecting
// client gets a new UUID. In production, the feiq++ relay client maintains a
// persistent WebSocket connection (with ping/pong and auto-reconnect), so offline
// messages are delivered as soon as the connection recovers.


//! Integration test: two Engine instances on different ports exchanging messages.
//! Tests the full protocol stack end-to-end without GUI.

use feiq_core::engine::engine::{build_ans_entry, build_br_entry, build_knock, build_text_message};
use feiq_core::network::NetworkEvent;
use feiq_core::protocol::constants::*;
use feiq_core::protocol::parser::ProtocolChain;
use feiq_core::protocol::serializer::parse_raw;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};

/// Thin wrapper: raw UDP socket + recv loop dispatching parsed events
struct TestPeer {
    socket: Arc<UdpSocket>,
    mac: String,
    name: String,
    ver: String,
    port: u16,
}

impl TestPeer {
    async fn new(name: &str, port: u16) -> Self {
        let socket = Arc::new(
            UdpSocket::bind(format!("127.0.0.1:{port}"))
                .await
                .expect("bind failed"),
        );
        socket.set_broadcast(true).ok();
        let mac = mac_address::get_mac_address()
            .ok()
            .flatten()
            .map(|m| m.to_string().replace(':', ""))
            .unwrap_or_default();
        let ver = format!("feiq_plus_plus#128#{mac}#0#0#0#1#9");
        Self {
            socket,
            mac,
            name: name.into(),
            ver,
            port,
        }
    }

    async fn send_to(&self, ip: &str, port: u16, data: &[u8]) {
        self.socket
            .send_to(data, format!("{ip}:{port}"))
            .await
            .expect("send failed");
    }

    /// Spawn recv loop, returns event receiver
    fn spawn_recv(
        self: Arc<Self>,
        event_tx: mpsc::UnboundedSender<NetworkEvent>,
    ) -> tokio::task::JoinHandle<()> {
        let socket = self.socket.clone();
        let mac = self.mac.clone();
        let name = self.name.clone();
        let chain = feiq_core::protocol::parser::build_default_chain();

        tokio::spawn(async move {
            let mut buf = vec![0u8; 4096];
            loop {
                match socket.recv_from(&mut buf).await {
                    Ok((len, addr)) => {
                        let data = buf[..len].to_vec();
                        let ip = addr.ip().to_string();
                        let port = addr.port();

                        // Parse raw packet (with sender's actual port)
                        let mut post = match parse_raw(&data, &ip, port, &mac, &name) {
                            Some(p) => p,
                            None => continue, // filtered or malformed
                        };

                        // Run protocol chain
                        chain.process(&mut post);

                        // Dispatch
                        if post.contents.is_empty() {
                            if feiq_core::protocol::constants::is_cmd_set(
                                post.cmd_id,
                                IPMSG_BR_ENTRY,
                            ) {
                                let _ = event_tx.send(NetworkEvent::FellowOnline(post));
                            } else if feiq_core::protocol::constants::is_cmd_set(
                                post.cmd_id,
                                IPMSG_BR_EXIT,
                            ) {
                                let _ = event_tx.send(NetworkEvent::FellowOffline(post));
                            } else if feiq_core::protocol::constants::is_cmd_set(
                                post.cmd_id,
                                IPMSG_ANSENTRY,
                            ) {
                                let _ = event_tx.send(NetworkEvent::FellowAnsEntry(post));
                            }
                        } else {
                            let _ = event_tx.send(NetworkEvent::Message(post));
                        }
                    }
                    Err(e) => {
                        eprintln!("recv error: {e}");
                        break;
                    }
                }
            }
        })
    }

    fn build_br_entry(&self) -> Vec<u8> {
        build_br_entry(&self.name, "test-host", &self.ver)
    }

    fn build_knock(&self) -> Vec<u8> {
        build_knock(&self.name, "test-host", &self.ver)
    }

    fn build_ans_entry(&self) -> Vec<u8> {
        build_ans_entry(&self.name, "test-host", &self.ver)
    }

    fn build_text(&self, packet_no: u64, text: &str) -> Vec<u8> {
        build_text_message(packet_no, &self.name, "test-host", &self.ver, text)
    }
}

#[tokio::test]
async fn test_two_instances_exchange_text() {
    let alice = Arc::new(TestPeer::new("Alice", 13551).await);
    let bob = Arc::new(TestPeer::new("Bob", 13552).await);

    let (alice_tx, mut alice_rx) = mpsc::unbounded_channel::<NetworkEvent>();
    let (bob_tx, mut bob_rx) = mpsc::unbounded_channel::<NetworkEvent>();

    let _a = alice.clone().spawn_recv(alice_tx);
    let _b = bob.clone().spawn_recv(bob_tx);
    sleep(Duration::from_millis(200)).await;

    println!("Alice: port {}, Bob: port {}", alice.port, bob.port);

    // ── Test 1: Alice sends text to Bob ──
    alice
        .send_to("127.0.0.1", bob.port, &alice.build_text(1001, "Hello Bob! /:)"))
        .await;

    let event = tokio::time::timeout(Duration::from_secs(2), bob_rx.recv())
        .await
        .expect("Timeout: Bob didn't receive text")
        .expect("Bob channel closed");

    match event {
        NetworkEvent::Message(post) => {
            assert_eq!(post.from.pc_name, "Alice");
            match &post.contents[0] {
                feiq_core::protocol::types::Content::Text { text, .. } => {
                    assert_eq!(text, "Hello Bob! /:)");
                }
                _ => panic!("Expected text"),
            }
        }
        _ => panic!("Expected Message"),
    }
    println!("✅ Test 1: Alice → Bob text with emoji");

    // ── Test 2: Bob sends knock to Alice ──
    bob.send_to("127.0.0.1", alice.port, &bob.build_knock())
        .await;

    let event = tokio::time::timeout(Duration::from_secs(2), alice_rx.recv())
        .await
        .expect("Timeout: Alice didn't receive knock")
        .expect("Alice channel closed");

    match event {
        NetworkEvent::Message(post) => {
            assert!(post.contents[0].is_knock());
            assert_eq!(post.from.pc_name, "Bob");
        }
        _ => panic!("Expected Message (knock)"),
    }
    println!("✅ Test 2: Bob → Alice window shake");

    // ── Test 3: Online discovery ──
    bob.send_to("127.0.0.1", alice.port, &bob.build_br_entry())
        .await;

    let event = tokio::time::timeout(Duration::from_secs(2), alice_rx.recv())
        .await
        .expect("Timeout: Alice didn't receive BR_ENTRY")
        .expect("Alice channel closed");

    match event {
        NetworkEvent::FellowOnline(post) => {
            assert_eq!(post.from.name, "Bob");
            assert!(post.from.online);
        }
        _ => panic!("Expected FellowOnline"),
    }
    println!("✅ Test 3: Bob → Alice online discovery");

    // ── Test 3b: Alice replies ANSENTRY to Bob (mutual discovery) ──
    alice
        .send_to("127.0.0.1", bob.port, &alice.build_ans_entry())
        .await;

    let event = tokio::time::timeout(Duration::from_secs(2), bob_rx.recv())
        .await
        .expect("Timeout: Bob didn't receive ANSENTRY")
        .expect("Bob channel closed");

    match event {
        NetworkEvent::FellowAnsEntry(post) => {
            assert_eq!(post.from.name, "Alice");
            assert!(post.from.online);
            // Verify Bob recorded Alice's actual port (not default)
            assert_eq!(post.from.port, alice.port,
                "ANSENTRY should preserve sender's actual port");
        }
        _ => panic!("Expected FellowAnsEntry"),
    }
    println!("✅ Test 3b: Alice → Bob ANSENTRY (mutual discovery, port={})", alice.port);

    // ── Test 3c: Verify port is not default when received from non-standard port ──
    let bob2 = Arc::new(TestPeer::new("BobNonStd", 13553).await);
    let (bob2_tx, mut bob2_rx) = mpsc::unbounded_channel::<NetworkEvent>();
    let _b2 = bob2.clone().spawn_recv(bob2_tx);
    sleep(Duration::from_millis(200)).await;

    // BobNonStd on port 13553 sends BR_ENTRY to Alice on port 13551
    bob2.send_to("127.0.0.1", alice.port, &bob2.build_br_entry()).await;

    let event = tokio::time::timeout(Duration::from_secs(2), alice_rx.recv())
        .await
        .expect("Timeout: Alice didn't receive BR_ENTRY from non-std port")
        .expect("Alice channel closed");

    match event {
        NetworkEvent::FellowOnline(post) => {
            assert_eq!(post.from.name, "BobNonStd");
            // Alice should record BobNonStd's real port (13553), not default 2425
            assert_eq!(post.from.port, 13553,
                "Port should be {} (sender's actual port), not 2425 (default)", 13553);
        }
        _ => panic!("Expected FellowOnline"),
    }
    println!("✅ Test 3c: Port preserved from non-standard port sender");

    // ── Test 4: Self-filter ──
    alice
        .send_to("127.0.0.1", alice.port, &alice.build_text(2001, "self"))
        .await;

    match tokio::time::timeout(Duration::from_secs(1), alice_rx.recv()).await {
        Ok(Some(NetworkEvent::Message(post))) => {
            panic!("Self-filter failed: received self-message from {}", post.from.name);
        }
        _ => {} // timeout = filtered, correct
    }
    println!("✅ Test 4: Self-filter works");

    println!("\n🎉 All 4 multi-instance integration tests passed!");
}

//! Integration test: protocol-level file notification flow.
//!
//! Two TestPeer instances communicate via UDP on localhost.
//! Peer A builds and sends a file notification message (IPMSG_SENDMSG with
//! IPMSG_FILEATTACHOPT). Peer B receives, parses through the protocol chain,
//! and verifies the extracted FileContent matches.
//!
//! Covers:
//! - ASCII / GBK-encoded Chinese filenames
//! - UTF-8 encoded Chinese filenames (IPMSG_UTF8OPT)
//! - Raw wire-format structure of the file notification
//! - Pure file-only notifications (no accompanying text)
//! - Self-filter: messages matching own MAC+name are dropped

use feiq_core::engine::engine::build_file_message;
use feiq_core::network::NetworkEvent;
use feiq_core::protocol::constants::*;
use feiq_core::protocol::serializer::pack_message;
use feiq_core::protocol::serializer::parse_raw;
use feiq_core::protocol::types::{Content, FileContent};
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};

/// Thin wrapper: raw UDP socket + recv loop dispatching parsed events.
/// Mirrors the TestPeer in multi_instance_test.rs.
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
        // Retrieve the actual bound port (the OS may have assigned a different
        // port if port was 0, though callers use explicit ports that must match).
        let bound_port = socket.local_addr().unwrap().port();
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
            port: bound_port,
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

                        let mut post = match parse_raw(&data, &ip, port, &mac, &name) {
                            Some(p) => p,
                            None => continue,
                        };

                        chain.process(&mut post);

                        if !post.contents.is_empty() {
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
}

// ─── Helpers ─────────────────────────────────────────────────────

/// Build a file notification with GBK-encoded filename (no UTF8OPT).
/// Delegates to the production `build_file_message`.
fn build_file_gbk(peer: &TestPeer, packet_no: u64, content: &FileContent) -> Vec<u8> {
    build_file_message(packet_no, &peer.name, "test-host", &peer.ver, content, false)
}

/// Build a file notification with UTF-8 filename and IPMSG_UTF8OPT.
/// Delegates to the production `build_file_message` with is_feiq_plus_plus=true.
fn build_file_utf8(peer: &TestPeer, packet_no: u64, content: &FileContent) -> Vec<u8> {
    build_file_message(packet_no, &peer.name, "test-host", &peer.ver, content, true)
}

// ─── Tests ───────────────────────────────────────────────────────

#[tokio::test]
async fn test_file_notification_gbk_ascii() {
    // ASCII filename with GBK encoding (GBK is transparent for ASCII)
    let alice = Arc::new(TestPeer::new("Alice", 13558).await);
    let bob = Arc::new(TestPeer::new("Bob", 13559).await);

    let (alice_tx, mut _alice_rx) = mpsc::unbounded_channel::<NetworkEvent>();
    let (bob_tx, mut bob_rx) = mpsc::unbounded_channel::<NetworkEvent>();

    let _a = alice.clone().spawn_recv(alice_tx);
    let _b = bob.clone().spawn_recv(bob_tx);
    sleep(Duration::from_millis(200)).await;

    let file = FileContent {
        file_id: 1001,
        filename: "report.pdf".into(),
        path: String::new(),
        size: 1048576,
        modify_time: 1700000000,
        file_type: 0,
        packet_no: 0,
        local_task_id: None,
    };

    let packet = build_file_gbk(&alice, 5001, &file);
    alice.send_to("127.0.0.1", bob.port, &packet).await;

    let event = tokio::time::timeout(Duration::from_secs(2), bob_rx.recv())
        .await
        .expect("Timeout: Bob did not receive file notification")
        .expect("Bob channel closed");

    match event {
        NetworkEvent::Message(post) => {
            assert_eq!(post.from.pc_name, "Alice");
            assert_eq!(
                post.contents.len(),
                1,
                "file-only notification should have 1 content entry"
            );
            match &post.contents[0] {
                Content::File(fc) => {
                    assert_eq!(fc.file_id, 1001);
                    assert_eq!(fc.filename, "report.pdf");
                    assert_eq!(fc.size, 1048576);
                    assert_eq!(fc.modify_time, 1700000000);
                    assert_eq!(fc.file_type, 0);
                }
                other => panic!("expected File content, got {:?}", other.content_type()),
            }
        }
        other => panic!("expected Message event, got {:?}", other),
    }
}

#[tokio::test]
async fn test_file_notification_gbk_chinese() {
    // Chinese filename encoded as GBK (no UTF8OPT). The parser decodes via
    // `decode_gbk` which correctly reconstructs the original Chinese text.
    let alice = Arc::new(TestPeer::new("Alice", 13560).await);
    let bob = Arc::new(TestPeer::new("Bob", 13561).await);

    let (alice_tx, _alice_rx) = mpsc::unbounded_channel::<NetworkEvent>();
    let (bob_tx, mut bob_rx) = mpsc::unbounded_channel::<NetworkEvent>();

    let _a = alice.clone().spawn_recv(alice_tx);
    let _b = bob.clone().spawn_recv(bob_tx);
    sleep(Duration::from_millis(200)).await;

    let file = FileContent {
        file_id: 2002,
        filename: "报告.pdf".into(),
        path: String::new(),
        size: 2048,
        modify_time: 1680000000,
        file_type: 1,
        packet_no: 0,
        local_task_id: None,
    };

    let packet = build_file_gbk(&alice, 5002, &file);
    alice.send_to("127.0.0.1", bob.port, &packet).await;

    let event = tokio::time::timeout(Duration::from_secs(2), bob_rx.recv())
        .await
        .expect("Timeout: Bob did not receive GBK Chinese file notification")
        .expect("Bob channel closed");

    match event {
        NetworkEvent::Message(post) => {
            assert_eq!(post.contents.len(), 1);
            match &post.contents[0] {
                Content::File(fc) => {
                    assert_eq!(fc.file_id, 2002);
                    assert_eq!(fc.filename, "报告.pdf");
                    assert_eq!(fc.size, 2048);
                    assert_eq!(fc.modify_time, 1680000000);
                    assert_eq!(fc.file_type, 1);
                }
                other => panic!("expected File content, got {:?}", other.content_type()),
            }
        }
        other => panic!("expected Message event, got {:?}", other),
    }
}

#[tokio::test]
async fn test_file_notification_utf8_chinese() {
    // Chinese filename encoded as UTF-8 with IPMSG_UTF8OPT set.
    // The parser's `parse_file_task` receives `is_utf8 = true` and decodes
    // via `decode_by_utf8opt(bytes, true)` which is a plain utf8 lossy decode,
    // preserving the original Chinese characters.
    let alice = Arc::new(TestPeer::new("Alice", 13562).await);
    let bob = Arc::new(TestPeer::new("Bob", 13563).await);

    let (alice_tx, _alice_rx) = mpsc::unbounded_channel::<NetworkEvent>();
    let (bob_tx, mut bob_rx) = mpsc::unbounded_channel::<NetworkEvent>();

    let _a = alice.clone().spawn_recv(alice_tx);
    let _b = bob.clone().spawn_recv(bob_tx);
    sleep(Duration::from_millis(200)).await;

    let file = FileContent {
        file_id: 3003,
        filename: "会议纪要.docx".into(),
        path: String::new(),
        size: 32768,
        modify_time: 1690000000,
        file_type: 2,
        packet_no: 0,
        local_task_id: None,
    };

    let packet = build_file_utf8(&alice, 5003, &file);
    alice.send_to("127.0.0.1", bob.port, &packet).await;

    let event = tokio::time::timeout(Duration::from_secs(2), bob_rx.recv())
        .await
        .expect("Timeout: Bob did not receive UTF-8 Chinese file notification")
        .expect("Bob channel closed");

    match event {
        NetworkEvent::Message(post) => {
            assert_eq!(post.contents.len(), 1);
            match &post.contents[0] {
                Content::File(fc) => {
                    assert_eq!(fc.file_id, 3003);
                    assert_eq!(fc.filename, "会议纪要.docx");
                    assert_eq!(fc.size, 32768);
                    assert_eq!(fc.modify_time, 1690000000);
                    assert_eq!(fc.file_type, 2);
                }
                other => panic!("expected File content, got {:?}", other.content_type()),
            }
        }
        other => panic!("expected Message event, got {:?}", other),
    }
}

#[tokio::test]
async fn test_file_notification_wire_format() {
    // Verify the raw wire format byte-by-byte without a network round-trip.
    let alice = Arc::new(TestPeer::new("Alice", 13564).await);

    let file = FileContent {
        file_id: 42,
        filename: "document.pdf".into(),
        path: String::new(),
        size: 0x1000,
        modify_time: 0xA5A5A5,
        file_type: 1,
        packet_no: 0,
        local_task_id: None,
    };

    let packet = build_file_gbk(&alice, 9001, &file);

    // ── Header ──────────────────────────────────────────────
    // Wire format:
    //   version:pktNo:Alice:test-host:cmdId:\0fileId:filename:size:mtime:ftype:\x07\0
    let hdr_prefix = format!("{}:9001:Alice:test-host:", alice.ver);
    assert!(
        packet.starts_with(hdr_prefix.as_bytes()),
        "packet must start with standard IPMSG header fields"
    );

    // ── Command-id ──────────────────────────────────────────
    let rest = &packet[hdr_prefix.len()..];
    let cmd_end = rest.iter().position(|&b| b == b':').expect("colon after cmd_id");
    let cmd_str = std::str::from_utf8(&rest[..cmd_end]).expect("cmd_id is valid UTF-8");
    let cmd_id: u32 = cmd_str.parse().expect("cmd_id is numeric");
    assert!(
        is_cmd_set(cmd_id, IPMSG_SENDMSG),
        "cmd_id must have SENDMSG set"
    );
    assert!(
        is_opt_set(cmd_id, IPMSG_FILEATTACHOPT),
        "cmd_id must have FILEATTACHOPT set"
    );

    // ── Body ────────────────────────────────────────────────
    let body_start = hdr_prefix.len() + cmd_end + 1; // skip "cmd_id:"
    let body = &packet[body_start..];

    // First byte is NULL (no accompanying text)
    assert_eq!(body[0], MSG_NULL, "body must start with null byte (no text)");

    // After the null: fileId:filename:size:mtime:ftype:\x07
    let file_entry = &body[1..]; // skip leading null
    let sep_pos = file_entry
        .iter()
        .position(|&b| b == FILELIST_SEPARATOR)
        .expect("file entry must end with FILELIST_SEPARATOR");

    let entry_str = std::str::from_utf8(&file_entry[..sep_pos]).expect("file entry is valid UTF-8");
    let fields: Vec<&str> = entry_str.split(':').collect();
    // Trailing colon after fileType produces 6 fields (5th is fileType, 6th is empty)
    assert_eq!(fields.len(), 6, "file entry has 5 colon-separated fields + trailing colon");
    assert_eq!(fields[0], "42", "file_id (decimal)");
    assert_eq!(fields[1], "document.pdf", "filename");
    assert_eq!(fields[2], "1000", "size in hex");
    assert_eq!(fields[3], "A5A5A5", "modify time in hex");
    assert_eq!(fields[4], "1", "file type in hex");
    assert!(fields[5].is_empty(), "trailing field after last colon is empty");

    // Trailing null from pack_message
    assert_eq!(
        packet.last(),
        Some(&MSG_NULL),
        "packet must end with null terminator"
    );
}

#[tokio::test]
async fn test_file_notification_no_text() {
    // A pure file notification (body starts with \0) must produce exactly 1
    // Content::File entry and NO Content::Text entry.
    let alice = Arc::new(TestPeer::new("Alice", 13565).await);
    let bob = Arc::new(TestPeer::new("Bob", 13566).await);

    let (alice_tx, _alice_rx) = mpsc::unbounded_channel::<NetworkEvent>();
    let (bob_tx, mut bob_rx) = mpsc::unbounded_channel::<NetworkEvent>();

    let _a = alice.clone().spawn_recv(alice_tx);
    let _b = bob.clone().spawn_recv(bob_tx);
    sleep(Duration::from_millis(200)).await;

    let file = FileContent {
        file_id: 77,
        filename: "data.bin".into(),
        path: String::new(),
        size: 500,
        modify_time: 100,
        file_type: 0,
        packet_no: 0,
        local_task_id: None,
    };

    let packet = build_file_gbk(&alice, 7777, &file);
    alice.send_to("127.0.0.1", bob.port, &packet).await;

    let event = tokio::time::timeout(Duration::from_secs(2), bob_rx.recv())
        .await
        .expect("Timeout: Bob did not receive file notification")
        .expect("Bob channel closed");

    match event {
        NetworkEvent::Message(post) => {
            // Only file content, no text
            assert_eq!(
                post.contents.len(),
                1,
                "file-only notification must produce exactly 1 content entry"
            );
            match &post.contents[0] {
                Content::File(fc) => {
                    assert_eq!(fc.file_id, 77);
                    assert_eq!(fc.filename, "data.bin");
                    assert_eq!(fc.size, 500);
                    assert_eq!(fc.modify_time, 100);
                    assert_eq!(fc.file_type, 0);
                }
                other => panic!("expected File content, got {:?}", other.content_type()),
            }
        }
        other => panic!("expected Message event, got {:?}", other),
    }
}

#[tokio::test]
async fn test_file_notification_with_text() {
    // A file notification that also has text before the null byte.
    // Both should be extracted: text + file content.
    let alice = Arc::new(TestPeer::new("Alice", 13567).await);
    let bob = Arc::new(TestPeer::new("Bob", 13568).await);

    let (alice_tx, _alice_rx) = mpsc::unbounded_channel::<NetworkEvent>();
    let (bob_tx, mut bob_rx) = mpsc::unbounded_channel::<NetworkEvent>();

    let _a = alice.clone().spawn_recv(alice_tx);
    let _b = bob.clone().spawn_recv(bob_tx);
    sleep(Duration::from_millis(200)).await;

    // Manually build a packet with text + file data
    let file_content = FileContent {
        file_id: 88,
        filename: "photo.jpg".into(),
        path: String::new(),
        size: 0x10000,
        modify_time: 0x123456,
        file_type: 1,
        packet_no: 0,
        local_task_id: None,
    };
    let filename_gbk = feiq_core::protocol::encoding::encode_gbk(
        &file_content.filename.replace(':', "::"),
    );

    let mut body = b"sending a file:"[..].to_vec(); // text before null
    body.push(MSG_NULL);
    let entry = format!(
        "{}:{}:{:X}:{:X}:{:X}:",
        file_content.file_id,
        String::from_utf8_lossy(&filename_gbk),
        file_content.size,
        file_content.modify_time,
        file_content.file_type,
    );
    body.extend_from_slice(entry.as_bytes());
    body.push(FILELIST_SEPARATOR);

    let packet = pack_message(
        8888,
        &alice.name,
        "test-host",
        &alice.ver,
        IPMSG_SENDMSG | IPMSG_FILEATTACHOPT,
        &body,
    );

    alice.send_to("127.0.0.1", bob.port, &packet).await;

    let event = tokio::time::timeout(Duration::from_secs(2), bob_rx.recv())
        .await
        .expect("Timeout: Bob did not receive file+text notification")
        .expect("Bob channel closed");

    match event {
        NetworkEvent::Message(post) => {
            assert_eq!(
                post.contents.len(),
                2,
                "notification with text + file must produce 2 content entries"
            );

            // First content: text
            match &post.contents[0] {
                Content::Text { text, .. } => {
                    assert_eq!(text, "sending a file:");
                }
                other => panic!("expected Text content, got {:?}", other.content_type()),
            }

            // Second content: file
            match &post.contents[1] {
                Content::File(fc) => {
                    assert_eq!(fc.file_id, 88);
                    assert_eq!(fc.filename, "photo.jpg");
                    assert_eq!(fc.size, 0x10000);
                    assert_eq!(fc.modify_time, 0x123456);
                    assert_eq!(fc.file_type, 1);
                }
                other => panic!("expected File content, got {:?}", other.content_type()),
            }
        }
        other => panic!("expected Message event, got {:?}", other),
    }
}

#[tokio::test]
async fn test_file_notification_multiple_entries() {
    // Multiple files in a single notification, separated by FILELIST_SEPARATOR.
    let alice = Arc::new(TestPeer::new("Alice", 13569).await);
    let bob = Arc::new(TestPeer::new("Bob", 13570).await);

    let (alice_tx, _alice_rx) = mpsc::unbounded_channel::<NetworkEvent>();
    let (bob_tx, mut bob_rx) = mpsc::unbounded_channel::<NetworkEvent>();

    let _a = alice.clone().spawn_recv(alice_tx);
    let _b = bob.clone().spawn_recv(bob_tx);
    sleep(Duration::from_millis(200)).await;

    // Manually build a notification with 2 file entries + text
    let mut body = b"multiple files:"[..].to_vec();
    body.push(MSG_NULL);
    // File 1: photo.jpg
    body.extend_from_slice(b"10:photo.jpg:8000:ABCD:1");
    body.push(FILELIST_SEPARATOR);
    // File 2: notes.txt
    body.extend_from_slice(b"20:notes.txt:200:1A:1");
    body.push(FILELIST_SEPARATOR);

    let packet = pack_message(
        9999,
        &alice.name,
        "test-host",
        &alice.ver,
        IPMSG_SENDMSG | IPMSG_FILEATTACHOPT,
        &body,
    );

    alice.send_to("127.0.0.1", bob.port, &packet).await;

    let event = tokio::time::timeout(Duration::from_secs(2), bob_rx.recv())
        .await
        .expect("Timeout: Bob did not receive multi-file notification")
        .expect("Bob channel closed");

    match event {
        NetworkEvent::Message(post) => {
            assert_eq!(
                post.contents.len(),
                3,
                "text + 2 files = 3 content entries"
            );

            // Text
            assert_eq!(post.contents[0].content_type(), "text");

            // File 1
            match &post.contents[1] {
                Content::File(fc) => {
                    assert_eq!(fc.file_id, 10);
                    assert_eq!(fc.filename, "photo.jpg");
                    assert_eq!(fc.size, 0x8000);
                }
                other => panic!("expected File, got {:?}", other.content_type()),
            }

            // File 2
            match &post.contents[2] {
                Content::File(fc) => {
                    assert_eq!(fc.file_id, 20);
                    assert_eq!(fc.filename, "notes.txt");
                    assert_eq!(fc.size, 0x200);
                }
                other => panic!("expected File, got {:?}", other.content_type()),
            }
        }
        other => panic!("expected Message event, got {:?}", other),
    }
}


#[tokio::test]
async fn test_file_notification_self_filter() {
    // A peer sending a file notification to itself (same MAC + name) must be
    // filtered out by the self-message filter in parse_raw.
    let alice = Arc::new(TestPeer::new("Alice", 13571).await);
    let (alice_tx, mut _alice_rx) = mpsc::unbounded_channel::<NetworkEvent>();
    let _a = alice.clone().spawn_recv(alice_tx.clone());

    // Need a second peer on a different port that shares alice's mac.
    // We can cheat by building the packet from Alice and sending to Alice's own
    // port -- then parse_raw sees the same MAC in the version string and the
    // same name in the header, and filters it out.
    let bob_on_alices_port = Arc::new(TestPeer::new("Bob", 13572).await);
    let (_bob_tx, _bob_rx) = mpsc::unbounded_channel::<NetworkEvent>();
    let _b = bob_on_alices_port.clone().spawn_recv(_bob_tx);

    // Wait for sockets to be ready
    sleep(Duration::from_millis(300)).await;

    let file = FileContent {
        file_id: 1,
        filename: "self.txt".into(),
        path: String::new(),
        size: 100,
        modify_time: 1000,
        file_type: 0,
        packet_no: 0,
        local_task_id: None,
    };

    // Alice builds a packet from herself, sends to herself
    let packet = build_file_gbk(&alice, 1111, &file);
    alice.send_to("127.0.0.1", alice.port, &packet).await;

    // The self-filter should drop it — nothing should arrive on alice's channel
    let result = tokio::time::timeout(Duration::from_secs(1), _alice_rx.recv()).await;
    match result {
        Ok(Some(NetworkEvent::Message(post))) => {
            panic!("Self-filter failed: received self-message from {}", post.from.name);
        }
        _ => {
            // Timeout or channel closed = self-filter works
        }
    }
}

// ─── Filename Edge Cases ─────────────────────────────────────────

#[tokio::test]
async fn test_file_notification_special_chars() {
    // Newline and emoji characters in filenames.
    // Newlines: tested via both GBK and UTF-8 paths.
    // Emoji: tested via UTF-8 path only (GBK cannot encode emoji).
    let alice = Arc::new(TestPeer::new("Alice", 13573).await);
    let bob = Arc::new(TestPeer::new("Bob", 13574).await);

    let (alice_tx, _alice_rx) = mpsc::unbounded_channel::<NetworkEvent>();
    let (bob_tx, mut bob_rx) = mpsc::unbounded_channel::<NetworkEvent>();

    let _a = alice.clone().spawn_recv(alice_tx);
    let _b = bob.clone().spawn_recv(bob_tx);
    sleep(Duration::from_millis(200)).await;

    // 1. Newline in filename via GBK path
    let file = FileContent {
        file_id: 40001,
        filename: "line1\nline2.txt".into(),
        path: String::new(),
        size: 1000,
        modify_time: 1700000000,
        file_type: 0,
        packet_no: 0,
        local_task_id: None,
    };
    let packet = build_file_gbk(&alice, 41001, &file);
    alice.send_to("127.0.0.1", bob.port, &packet).await;
    let event = tokio::time::timeout(Duration::from_secs(2), bob_rx.recv())
        .await
        .expect("Timeout: newline GBK notification not received")
        .expect("Bob channel closed");
    match &event {
        NetworkEvent::Message(post) => {
            assert_eq!(post.contents.len(), 1);
            match &post.contents[0] {
                Content::File(fc) => {
                    assert_eq!(fc.file_id, 40001);
                    assert_eq!(fc.filename, "line1\nline2.txt");
                    assert_eq!(fc.size, 1000);
                }
                other => panic!("expected File, got {:?}", other.content_type()),
            }
        }
        other => panic!("expected Message, got {:?}", other),
    }

    // 2. Newline in filename via UTF-8 path
    let file = FileContent {
        file_id: 40002,
        filename: "hello\nworld.docx".into(),
        path: String::new(),
        size: 2048,
        modify_time: 1700000001,
        file_type: 1,
        packet_no: 0,
        local_task_id: None,
    };
    let packet = build_file_utf8(&alice, 41002, &file);
    alice.send_to("127.0.0.1", bob.port, &packet).await;
    let event = tokio::time::timeout(Duration::from_secs(2), bob_rx.recv())
        .await
        .expect("Timeout: newline UTF-8 notification not received")
        .expect("Bob channel closed");
    match &event {
        NetworkEvent::Message(post) => {
            assert_eq!(post.contents.len(), 1);
            match &post.contents[0] {
                Content::File(fc) => {
                    assert_eq!(fc.file_id, 40002);
                    assert_eq!(fc.filename, "hello\nworld.docx");
                    assert_eq!(fc.size, 2048);
                }
                other => panic!("expected File, got {:?}", other.content_type()),
            }
        }
        other => panic!("expected Message, got {:?}", other),
    }

    // 3. Emoji in filename via UTF-8 path
    let file = FileContent {
        file_id: 40003,
        filename: "report_😊_final🎉.pdf".into(),
        path: String::new(),
        size: 4096,
        modify_time: 1700000002,
        file_type: 2,
        packet_no: 0,
        local_task_id: None,
    };
    let packet = build_file_utf8(&alice, 41003, &file);
    alice.send_to("127.0.0.1", bob.port, &packet).await;
    let event = tokio::time::timeout(Duration::from_secs(2), bob_rx.recv())
        .await
        .expect("Timeout: emoji UTF-8 notification not received")
        .expect("Bob channel closed");
    match &event {
        NetworkEvent::Message(post) => {
            assert_eq!(post.contents.len(), 1);
            match &post.contents[0] {
                Content::File(fc) => {
                    assert_eq!(fc.file_id, 40003);
                    assert_eq!(fc.filename, "report_😊_final🎉.pdf");
                    assert_eq!(fc.size, 4096);
                }
                other => panic!("expected File, got {:?}", other.content_type()),
            }
        }
        other => panic!("expected Message, got {:?}", other),
    }
}

#[tokio::test]
async fn test_file_notification_long_filename() {
    // 500-char ASCII filename via both GBK and UTF-8 paths.
    // Verifies that oversize filenames survive build -> wire -> parse roundtrip.
    let alice = Arc::new(TestPeer::new("Alice", 13575).await);
    let bob = Arc::new(TestPeer::new("Bob", 13576).await);
    let (alice_tx, _alice_rx) = mpsc::unbounded_channel::<NetworkEvent>();
    let (bob_tx, mut bob_rx) = mpsc::unbounded_channel::<NetworkEvent>();
    let _a = alice.clone().spawn_recv(alice_tx);
    let _b = bob.clone().spawn_recv(bob_tx);
    sleep(Duration::from_millis(200)).await;

    let long_name = "a".repeat(500) + ".txt";
    assert_eq!(long_name.len(), 504);

    // UTF-8 path
    let file = FileContent {
        file_id: 50001,
        filename: long_name.clone(),
        path: String::new(),
        size: 0xABCD,
        modify_time: 0x12345678,
        file_type: 1,
        packet_no: 0,
        local_task_id: None,
    };
    let packet = build_file_utf8(&alice, 51001, &file);
    assert!(
        packet.len() < 4000,
        "UTF-8 packet too large for 4096-byte recv buffer: {}",
        packet.len()
    );
    alice.send_to("127.0.0.1", bob.port, &packet).await;
    let event = tokio::time::timeout(Duration::from_secs(2), bob_rx.recv())
        .await
        .expect("Timeout: long filename UTF-8")
        .expect("Bob channel closed");
    match &event {
        NetworkEvent::Message(post) => {
            assert_eq!(post.contents.len(), 1);
            match &post.contents[0] {
                Content::File(fc) => {
                    assert_eq!(fc.file_id, 50001);
                    assert_eq!(fc.filename, long_name);
                    assert_eq!(fc.size, 0xABCD);
                    assert_eq!(fc.modify_time, 0x12345678);
                }
                other => panic!("expected File, got {:?}", other.content_type()),
            }
        }
        other => panic!("expected Message, got {:?}", other),
    }

    // GBK path (ASCII filename produces identical bytes)
    let packet_gbk = build_file_gbk(&alice, 51002, &file);
    assert!(
        packet_gbk.len() < 4000,
        "GBK packet too large for 4096-byte recv buffer: {}",
        packet_gbk.len()
    );
    alice.send_to("127.0.0.1", bob.port, &packet_gbk).await;
    let event = tokio::time::timeout(Duration::from_secs(2), bob_rx.recv())
        .await
        .expect("Timeout: long filename GBK")
        .expect("Bob channel closed");
    match &event {
        NetworkEvent::Message(post) => {
            assert_eq!(post.contents.len(), 1);
            match &post.contents[0] {
                Content::File(fc) => {
                    assert_eq!(fc.file_id, 50001);
                    assert_eq!(fc.filename, long_name);
                    assert_eq!(fc.size, 0xABCD);
                }
                other => panic!("expected File, got {:?}", other.content_type()),
            }
        }
        other => panic!("expected Message, got {:?}", other),
    }
}

#[tokio::test]
async fn test_file_notification_file_id_zero() {
    // file_id = 0 is a valid protocol value and must be preserved
    // through the full build -> wire -> parse roundtrip.
    let alice = Arc::new(TestPeer::new("Alice", 13577).await);
    let bob = Arc::new(TestPeer::new("Bob", 13578).await);
    let (alice_tx, _alice_rx) = mpsc::unbounded_channel::<NetworkEvent>();
    let (bob_tx, mut bob_rx) = mpsc::unbounded_channel::<NetworkEvent>();
    let _a = alice.clone().spawn_recv(alice_tx);
    let _b = bob.clone().spawn_recv(bob_tx);
    sleep(Duration::from_millis(200)).await;

    // UTF-8 path
    let file = FileContent {
        file_id: 0,
        filename: "zeroid.dat".into(),
        path: String::new(),
        size: 777,
        modify_time: 999,
        file_type: 0,
        packet_no: 0,
        local_task_id: None,
    };
    let packet = build_file_utf8(&alice, 71001, &file);
    alice.send_to("127.0.0.1", bob.port, &packet).await;
    let event = tokio::time::timeout(Duration::from_secs(2), bob_rx.recv())
        .await
        .expect("Timeout: file_id=0 UTF-8")
        .expect("Bob channel closed");
    match &event {
        NetworkEvent::Message(post) => {
            assert_eq!(post.contents.len(), 1);
            match &post.contents[0] {
                Content::File(fc) => {
                    assert_eq!(fc.file_id, 0, "file_id must be preserved as 0");
                    assert_eq!(fc.filename, "zeroid.dat");
                    assert_eq!(fc.size, 777);
                }
                other => panic!("expected File, got {:?}", other.content_type()),
            }
        }
        other => panic!("expected Message, got {:?}", other),
    }

    // Also test via GBK path
    let packet_gbk = build_file_gbk(&alice, 71002, &file);
    alice.send_to("127.0.0.1", bob.port, &packet_gbk).await;
    let event = tokio::time::timeout(Duration::from_secs(2), bob_rx.recv())
        .await
        .expect("Timeout: file_id=0 GBK")
        .expect("Bob channel closed");
    match &event {
        NetworkEvent::Message(post) => {
            assert_eq!(post.contents.len(), 1);
            match &post.contents[0] {
                Content::File(fc) => {
                    assert_eq!(fc.file_id, 0, "file_id must be preserved as 0 via GBK path");
                    assert_eq!(fc.filename, "zeroid.dat");
                    assert_eq!(fc.size, 777);
                }
                other => panic!("expected File, got {:?}", other.content_type()),
            }
        }
        other => panic!("expected Message, got {:?}", other),
    }
}

#[test]
fn test_file_notification_negative_size() {
    // Negative size: build_file_message formats the i64 size field with {:X}.
    // For negative values this produces the full 64-bit two's complement
    // representation (FFFFFFFFFFFFFFFF for -1). parse_file_task reads it
    // back via i64::from_str_radix(val, 16), which rejects values exceeding
    // i64::MAX with PosOverflow. Consequently the file entry is silently dropped.
    let file = FileContent {
        file_id: 60001,
        filename: "neg_size.bin".into(),
        path: String::new(),
        size: -1,
        modify_time: 0xABCD,
        file_type: 0,
        packet_no: 0,
        local_task_id: None,
    };
    let packet = build_file_message(
        61001,
        "Alice",
        "test-host",
        "feiq_plus_plus#128#MAC0001#0#0#0#1#9",
        &file,
        true,
    );

    // Verify wire format: negative -1 is serialized as FFFFFFFFFFFFFFFF
    let packet_str = String::from_utf8_lossy(&packet);
    assert!(
        packet_str.contains("FFFFFFFFFFFFFFFF"),
        "Negative size -1 should be formatted as FFFFFFFFFFFFFFFF, got: {packet_str}"
    );
    // The hex size must appear as the field immediately after filename
    let body_part = &packet_str[packet_str.find("neg_size.bin").unwrap_or(0)..];
    assert!(
        body_part.starts_with("neg_size.bin:FFFFFFFFFFFFFFFF:"),
        "Size field should be immediately after filename: {body_part}"
    );

    // Parse through the full protocol chain
    let mut post = parse_raw(&packet, "10.0.0.1", 2425, "OTHER_MAC", "Bob")
        .expect("Packet should be parseable as valid IPMSG");
    let chain = feiq_core::protocol::parser::build_default_chain();
    chain.process(&mut post);

    assert!(
        post.contents.is_empty(),
        "Negative size FFFFFFFFFFFFFFFF overflows i64::from_str_radix, \
         so parse_file_task returns None and the file entry is silently dropped. \
         Got {} content(s)",
        post.contents.len()
    );
}

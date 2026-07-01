//! End-to-end integration test for folder transfer.
//!
//! Tests the full protocol flow end-to-end with two TestPeer instances:
//! 1. UDP: Sender sends folder notification (IPMSG_FILE_DIR + `__FOLDER__` prefix)
//! 2. TCP: Receiver connects, requests manifest, receives manifest + all files
//! 3. Content verification: every byte matches the original
//!
//! The TCP server side mirrors the engine's TCP accept loop logic but is
//! kept explicit in the test so the flow is fully transparent.

use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};

use feiq_core::engine::engine::build_file_message;
use feiq_core::network::tcp::{build_folder_manifest, FileTransfer};
use feiq_core::network::NetworkEvent;
use feiq_core::protocol::constants::*;
use feiq_core::protocol::types::{Content, FileContent, FolderManifest};

// ─── TestPeer (mirrors the pattern in file_transfer_test.rs) ───────────

/// Thin wrapper: raw UDP socket + recv loop dispatching parsed events.
struct TestPeer {
    socket: Arc<tokio::net::UdpSocket>,
    mac: String,
    name: String,
    ver: String,
    port: u16,
}

impl TestPeer {
    async fn new(name: &str, port: u16) -> Self {
        let socket = Arc::new(
            tokio::net::UdpSocket::bind(format!("127.0.0.1:{port}"))
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

                        let mut post = match feiq_core::protocol::serializer::parse_raw(
                            &data, &ip, port, &mac, &name,
                        ) {
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

// ─── Temp directory helpers ───────────────────────────────────────────

/// Create a temporary directory for use as the sender's folder.
/// Returns (path_string, PathBuf).
fn create_temp_folder() -> (String, std::path::PathBuf) {
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let tid = std::thread::current().id();
    let dir = std::path::PathBuf::from(format!("/tmp/feiq_folder_test_sender_{pid}_{tid:?}_{nanos}"));
    // Ensure uniqueness by retrying on collision
    if dir.exists() {
        std::thread::sleep(std::time::Duration::from_millis(1));
        return create_temp_folder();
    }
    std::fs::create_dir_all(&dir).unwrap();
    (dir.to_string_lossy().to_string(), dir)
}

/// Create a temporary download directory for the receiver.
fn create_download_dir() -> (String, std::path::PathBuf) {
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let tid = std::thread::current().id();
    let dir = std::path::PathBuf::from(format!("/tmp/feiq_folder_test_receiver_{pid}_{tid:?}_{nanos}"));
    if dir.exists() {
        std::thread::sleep(std::time::Duration::from_millis(1));
        return create_download_dir();
    }
    std::fs::create_dir_all(&dir).unwrap();
    (dir.to_string_lossy().to_string(), dir)
}

// ─── TCP sender/receiver helpers ──────────────────────────────────────

/// Sender-side folder transfer handler.
///
/// Mirrors the engine's TCP accept loop: reads FOLDER_MANIFEST_REQUEST,
/// rebuilds the manifest from disk, sends manifest, then for each file
/// sends header, waits for ACK, sends file content, and finally sends
/// the completion marker.
async fn run_folder_sender(
    stream: TcpStream,
    folder_path: &str,
    transfer_id: u64,
) -> anyhow::Result<FolderStats> {
    let mut ft = FileTransfer::from_stream(stream);

    // Read the manifest request
    let mut peek_buf = [0u8; 32];
    let n = ft.recv(&mut peek_buf).await?;
    let request = String::from_utf8_lossy(&peek_buf[..n]);
    assert!(
        request.contains("FOLDER_MANIFEST_REQUEST"),
        "first message must be manifest request, got: {request}"
    );

    // Rebuild manifest from disk
    let manifest = build_folder_manifest(folder_path, transfer_id)
        .expect("folder must exist and contain files");
    let total_files = manifest.total_files;
    let total_bytes = manifest.total_bytes;

    // Send manifest
    ft.send_folder_manifest(&manifest).await?;

    // Send each file
    let mut completed_files: u32 = 0;
    let mut overall_bytes: u64 = 0;
    for entry in &manifest.files {
        // Send file header
        ft.send_folder_file_header(&entry.relative_path, entry.size)
            .await?;

        // Wait for ACK
        let mut ack_buf = [0u8; FOLDER_FILE_ACK.len()];
        let _ = ft.recv(&mut ack_buf).await;

        // Send file content
        let full_path = std::path::Path::new(folder_path).join(&entry.relative_path);
        let full_path_str = full_path.to_string_lossy().to_string();
        ft.send_file(&full_path_str, 0, |_, _| {}, None).await?;

        overall_bytes += entry.size;
        completed_files += 1;
    }

    // Send completion marker
    ft.send(FOLDER_TRANSFER_COMPLETE).await?;

    Ok(FolderStats {
        total_files,
        total_bytes,
        completed_files,
        overall_bytes,
    })
}

/// Stats from a completed folder transfer.
#[derive(Debug)]
#[allow(dead_code)]
struct FolderStats {
    total_files: u32,
    total_bytes: u64,
    completed_files: u32,
    overall_bytes: u64,
}

// ─── Helpers ──────────────────────────────────────────────────────────

/// Build folder notification UDP packet (the `__FOLDER__` prefix convention).
fn build_folder_notification(
    peer: &TestPeer,
    packet_no: u64,
    folder_path: &str,
    folder_name: &str,
    file_count: u32,
    total_size: u64,
) -> Vec<u8> {
    let folder_meta = serde_json::json!({
        "name": folder_name,
        "count": file_count,
        "size": total_size,
        "tid": packet_no,
    });
    let folder_filename = format!("__FOLDER__{folder_meta}");

    let fc = FileContent {
        file_id: packet_no,
        filename: folder_filename,
        path: folder_path.to_string(),
        size: total_size as i64,
        modify_time: 0,
        file_type: IPMSG_FILE_DIR,
        packet_no,
        local_task_id: None,
    };

    build_file_message(
        packet_no,
        &peer.name,
        "test-host",
        &peer.ver,
        &fc,
        true, // feiq++ only
    )
}

/// Compare every file under two directories, verifying they are identical.
fn verify_directory_contents(original_dir: &str, downloaded_dir: &str, manifest: &FolderManifest) {
    for entry in &manifest.files {
        let original_path = std::path::Path::new(original_dir).join(&entry.relative_path);
        let downloaded_path = std::path::Path::new(downloaded_dir).join(&entry.relative_path);

        assert!(
            original_path.exists(),
            "original file must exist: {}",
            original_path.display()
        );
        assert!(
            downloaded_path.exists(),
            "downloaded file must exist: {}",
            downloaded_path.display()
        );

        let original_bytes = std::fs::read(&original_path).unwrap_or_else(|e| {
            panic!("failed to read original {}: {e}", original_path.display())
        });
        let downloaded_bytes = std::fs::read(&downloaded_path).unwrap_or_else(|e| {
            panic!(
                "failed to read downloaded {}: {e}",
                downloaded_path.display()
            )
        });

        assert_eq!(
            downloaded_bytes.len(),
            entry.size as usize,
            "file size mismatch for {}",
            entry.relative_path
        );
        assert_eq!(
            downloaded_bytes, original_bytes,
            "content mismatch for {}",
            entry.relative_path
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════
//  Tests
// ═══════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_folder_transfer_e2e_basic() {
    // ─── Setup: create sender folder with 3 files ───────────────
    let (sender_path, sender_dir) = create_temp_folder();
    let (download_path, download_dir) = create_download_dir();

    std::fs::write(sender_dir.join("readme.txt"), b"Hello, Folder!").unwrap();
    std::fs::write(sender_dir.join("data.bin"), &[0xABu8; 1000]).unwrap();
    std::fs::write(sender_dir.join("notes.md"), b"# Notes\n\nThis is a test.").unwrap();

    let transfer_id: u64 = 79001;
    let manifest = build_folder_manifest(&sender_path, transfer_id)
        .expect("manifest must build");
    assert_eq!(manifest.total_files, 3);
    assert_eq!(manifest.folder_name, sender_dir.file_name().unwrap().to_str().unwrap());

    // ─── UDP: folder notification ──────────────────────────────
    let alice = Arc::new(TestPeer::new("Alice", 14101).await);
    let bob = Arc::new(TestPeer::new("Bob", 14102).await);

    let (alice_tx, _alice_rx) = mpsc::unbounded_channel::<NetworkEvent>();
    let (bob_tx, mut bob_rx) = mpsc::unbounded_channel::<NetworkEvent>();

    let _a = alice.clone().spawn_recv(alice_tx);
    let _b = bob.clone().spawn_recv(bob_tx);
    sleep(Duration::from_millis(200)).await;

    // Send folder notification
    let packet = build_folder_notification(
        &alice,
        transfer_id,
        &sender_path,
        manifest.folder_name.as_str(),
        manifest.total_files,
        manifest.total_bytes,
    );
    alice.send_to("127.0.0.1", bob.port, &packet).await;

    // Bob receives folder notification
    let event = tokio::time::timeout(Duration::from_secs(2), bob_rx.recv())
        .await
        .expect("Timeout: Bob did not receive folder notification")
        .expect("Bob channel closed");

    let (bob_recv_transfer_id, bob_recv_folder_name) = match event {
        NetworkEvent::Message(post) => {
            assert_eq!(
                post.contents.len(),
                1,
                "folder notification must produce 1 content entry"
            );
            let fc = match &post.contents[0] {
                Content::File(fc) => fc,
                other => panic!("expected Content::File, got {:?}", other.content_type()),
            };
            assert_eq!(
                fc.file_type, IPMSG_FILE_DIR,
                "folder notification must have IPMSG_FILE_DIR type"
            );
            assert!(
                fc.filename.starts_with("__FOLDER__"),
                "folder notification filename must start with __FOLDER__"
            );
            // Verify the packet_no matches for TCP handshake
            let pkt: u64 = post.packet_no.parse().unwrap_or(0);
            assert_eq!(pkt, transfer_id, "packet_no must match transfer_id");
            (pkt, fc.filename.clone())
        }
        other => panic!("expected Message event, got {:?}", other),
    };
    assert_eq!(bob_recv_transfer_id, transfer_id);
    assert!(bob_recv_folder_name.contains(&manifest.folder_name));

    println!("✅ UDP notification: Alice → Bob (folder: {})", manifest.folder_name);

    // ─── TCP: folder data transfer ─────────────────────────────
    // Alice starts TCP listener
    let tcp_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let tcp_addr = tcp_listener.local_addr().unwrap();

    let sender_path_clone = sender_path.clone();
    let tcp_handle = tokio::spawn(async move {
        let (stream, _) = tcp_listener.accept().await.expect("TCP accept");
        run_folder_sender(stream, &sender_path_clone, transfer_id)
            .await
            .expect("folder sender failed")
    });

    // Bob connects and receives folder
    let download_path_clone = download_path.clone();
    let receiver_handle = tokio::spawn(async move {
        let mut bob_ft = FileTransfer::connect("127.0.0.1", tcp_addr.port())
            .await
            .expect("Bob TCP connect");
        let request = format!("FOLDER_MANIFEST_REQUEST\n{transfer_id}\n");
        bob_ft.send(request.as_bytes()).await.unwrap();
        let manifest = bob_ft.recv_folder_manifest().await.unwrap();

        for entry in &manifest.files {
            let (path, size) = bob_ft.recv_folder_file_header().await.unwrap();
            assert_eq!(path, entry.relative_path);
            assert_eq!(size, entry.size);
            bob_ft.send(b"FOLDER_FILE_ACK\n").await.unwrap();
            let out_path = std::path::Path::new(&download_path_clone).join(&path);
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            bob_ft
                .recv_file(&out_path.to_string_lossy(), size as i64, |_, _| {}, None)
                .await
                .unwrap();
        }

        let mut done_buf = [0u8; 32];
        bob_ft.recv(&mut done_buf).await.unwrap();
        manifest
    });

    let sender_stats = tcp_handle.await.unwrap();
    let recv_manifest = receiver_handle.await.unwrap();

    // ─── Verify ────────────────────────────────────────────────
    assert_eq!(sender_stats.total_files, 3);
    assert_eq!(sender_stats.completed_files, 3);
    assert_eq!(recv_manifest.total_files, 3);
    assert_eq!(recv_manifest.transfer_id, transfer_id);

    verify_directory_contents(&sender_path, &download_path, &recv_manifest);

    println!(
        "✅ TCP transfer: {}/{} files, {} bytes",
        recv_manifest.total_files,
        sender_stats.completed_files,
        recv_manifest.total_bytes
    );

    // Cleanup
    let _ = std::fs::remove_dir_all(&sender_dir);
    let _ = std::fs::remove_dir_all(&download_dir);
}

#[tokio::test]
async fn test_folder_transfer_e2e_nested() {
    // ─── Setup: create nested folder structure ──────────────────
    let (sender_path, sender_dir) = create_temp_folder();
    let (download_path, download_dir) = create_download_dir();

    // Create nested structure:
    //   root.txt
    //   sub1/hello.txt
    //   sub1/sub2/deep.bin
    //   sub1/sub2/sub3/leaf.dat
    //   images/photo.jpg
    std::fs::write(sender_dir.join("root.txt"), b"root level file").unwrap();
    std::fs::create_dir_all(sender_dir.join("sub1/sub2/sub3")).unwrap();
    std::fs::write(sender_dir.join("sub1/hello.txt"), b"hello from sub1").unwrap();
    std::fs::write(sender_dir.join("sub1/sub2/deep.bin"), &[0x01u8; 500]).unwrap();
    std::fs::write(
        sender_dir.join("sub1/sub2/sub3/leaf.dat"),
        b"deeply nested",
    )
    .unwrap();
    std::fs::create_dir_all(sender_dir.join("images")).unwrap();
    std::fs::write(sender_dir.join("images/photo.jpg"), &[0xFFu8; 2048]).unwrap();

    let transfer_id: u64 = 79002;
    let manifest = build_folder_manifest(&sender_path, transfer_id)
        .expect("manifest must build");
    assert_eq!(manifest.total_files, 5);
    assert_eq!(manifest.files.len(), 5);

    // ─── UDP: folder notification ──────────────────────────────
    let alice = Arc::new(TestPeer::new("Alice", 14103).await);
    let bob = Arc::new(TestPeer::new("Bob", 14104).await);

    let (alice_tx, _alice_rx) = mpsc::unbounded_channel::<NetworkEvent>();
    let (bob_tx, mut bob_rx) = mpsc::unbounded_channel::<NetworkEvent>();

    let _a = alice.clone().spawn_recv(alice_tx);
    let _b = bob.clone().spawn_recv(bob_tx);
    sleep(Duration::from_millis(200)).await;

    let packet = build_folder_notification(
        &alice,
        transfer_id,
        &sender_path,
        &manifest.folder_name,
        manifest.total_files,
        manifest.total_bytes,
    );
    alice.send_to("127.0.0.1", bob.port, &packet).await;

    let event = tokio::time::timeout(Duration::from_secs(2), bob_rx.recv())
        .await
        .expect("Timeout: Bob did not receive folder notification")
        .expect("Bob channel closed");

    match event {
        NetworkEvent::Message(post) => {
            assert_eq!(post.contents.len(), 1);
            let fc = match &post.contents[0] {
                Content::File(fc) => fc,
                other => panic!("expected Content::File, got {:?}", other.content_type()),
            };
            assert_eq!(fc.file_type, IPMSG_FILE_DIR);
            assert!(fc.filename.starts_with("__FOLDER__"));
        }
        other => panic!("expected Message event, got {:?}", other),
    }

    println!("✅ UDP notification: Alice → Bob (nested folder)");

    // ─── TCP: folder data transfer ─────────────────────────────
    let tcp_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let tcp_addr = tcp_listener.local_addr().unwrap();

    let sender_path_clone = sender_path.clone();
    let tcp_handle = tokio::spawn(async move {
        let (stream, _) = tcp_listener.accept().await.expect("TCP accept");
        run_folder_sender(stream, &sender_path_clone, transfer_id)
            .await
            .expect("folder sender failed")
    });

    let download_path_clone = download_path.clone();
    let receiver_handle = tokio::spawn(async move {
        let mut bob_ft =
            FileTransfer::connect("127.0.0.1", tcp_addr.port())
                .await
                .expect("Bob TCP connect");
        let request = format!("FOLDER_MANIFEST_REQUEST\n{transfer_id}\n");
        bob_ft.send(request.as_bytes()).await.unwrap();
        let manifest = bob_ft.recv_folder_manifest().await.unwrap();

        for entry in &manifest.files {
            let (path, size) = bob_ft.recv_folder_file_header().await.unwrap();
            assert_eq!(path, entry.relative_path, "path mismatch for {}", path);
            assert_eq!(size, entry.size, "size mismatch for {}", path);
            bob_ft.send(b"FOLDER_FILE_ACK\n").await.unwrap();
            let out_path = std::path::Path::new(&download_path_clone).join(&path);
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            bob_ft
                .recv_file(&out_path.to_string_lossy(), size as i64, |_, _| {}, None)
                .await
                .unwrap();
        }

        let mut done_buf = [0u8; 32];
        bob_ft.recv(&mut done_buf).await.unwrap();
        manifest
    });

    let sender_stats = tcp_handle.await.unwrap();
    let recv_manifest = receiver_handle.await.unwrap();

    // ─── Verify ────────────────────────────────────────────────
    assert_eq!(sender_stats.total_files, 5);
    assert_eq!(recv_manifest.total_files, 5);
    assert_eq!(sender_stats.completed_files, 5);

    // Verify directory structure preserved
    let expected_paths = [
        "root.txt",
        "sub1/hello.txt",
        "sub1/sub2/deep.bin",
        "sub1/sub2/sub3/leaf.dat",
        "images/photo.jpg",
    ];
    for path in &expected_paths {
        assert!(
            recv_manifest.files.iter().any(|f| f.relative_path == *path),
            "manifest must contain entry for {path}"
        );
        assert!(
            download_dir.join(path).exists(),
            "downloaded file must exist: {path}"
        );
    }

    // Verify content integrity
    verify_directory_contents(&sender_path, &download_path, &recv_manifest);

    println!(
        "✅ Nested folder transfer: {}/{} files, structure preserved",
        recv_manifest.total_files, sender_stats.completed_files
    );

    // Cleanup
    let _ = std::fs::remove_dir_all(&sender_dir);
    let _ = std::fs::remove_dir_all(&download_dir);
}

#[tokio::test]
async fn test_folder_transfer_e2e_large_batch() {
    // ─── Setup: create 100 small files ──────────────────────────
    let (sender_path, sender_dir) = create_temp_folder();
    let (download_path, download_dir) = create_download_dir();

    // Create 100 files in 2 subdirectories
    std::fs::create_dir_all(sender_dir.join("group_a")).unwrap();
    std::fs::create_dir_all(sender_dir.join("group_b")).unwrap();

    let mut expected_total: u64 = 0;
    for i in 0..100 {
        let content = format!("file_{}_content_{}_trailer", i, "x".repeat(i % 50));
        expected_total += content.len() as u64;
        let group = if i < 50 { "group_a" } else { "group_b" };
        std::fs::write(sender_dir.join(group).join(format!("f{i:04}.dat")), &content).unwrap();
    }

    let transfer_id: u64 = 79003;
    let manifest = build_folder_manifest(&sender_path, transfer_id)
        .expect("manifest must build");
    assert_eq!(manifest.total_files, 100);
    assert_eq!(manifest.total_bytes, expected_total);

    // ─── UDP: folder notification ──────────────────────────────
    let alice = Arc::new(TestPeer::new("Alice", 14105).await);
    let bob = Arc::new(TestPeer::new("Bob", 14106).await);

    let (alice_tx, _alice_rx) = mpsc::unbounded_channel::<NetworkEvent>();
    let (bob_tx, mut bob_rx) = mpsc::unbounded_channel::<NetworkEvent>();

    let _a = alice.clone().spawn_recv(alice_tx);
    let _b = bob.clone().spawn_recv(bob_tx);
    sleep(Duration::from_millis(200)).await;

    let packet = build_folder_notification(
        &alice,
        transfer_id,
        &sender_path,
        &manifest.folder_name,
        manifest.total_files,
        manifest.total_bytes,
    );
    alice.send_to("127.0.0.1", bob.port, &packet).await;

    let event = tokio::time::timeout(Duration::from_secs(2), bob_rx.recv())
        .await
        .expect("Timeout: Bob did not receive folder notification")
        .expect("Bob channel closed");

    match event {
        NetworkEvent::Message(post) => {
            assert_eq!(post.contents.len(), 1);
            let fc = match &post.contents[0] {
                Content::File(fc) => fc,
                other => panic!("expected Content::File, got {:?}", other.content_type()),
            };
            assert_eq!(fc.file_type, IPMSG_FILE_DIR);
            assert!(fc.filename.starts_with("__FOLDER__"));
        }
        other => panic!("expected Message event, got {:?}", other),
    }

    println!("✅ UDP notification: 100-file folder");

    // ─── TCP: folder data transfer ─────────────────────────────
    let tcp_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let tcp_addr = tcp_listener.local_addr().unwrap();

    let sender_path_clone = sender_path.clone();
    let tcp_handle = tokio::spawn(async move {
        let (stream, _) = tcp_listener.accept().await.expect("TCP accept");
        run_folder_sender(stream, &sender_path_clone, transfer_id)
            .await
            .expect("folder sender failed")
    });

    let download_path_clone = download_path.clone();
    let receiver_handle = tokio::spawn(async move {
        let mut bob_ft =
            FileTransfer::connect("127.0.0.1", tcp_addr.port())
                .await
                .expect("Bob TCP connect");
        let request = format!("FOLDER_MANIFEST_REQUEST\n{transfer_id}\n");
        bob_ft.send(request.as_bytes()).await.unwrap();
        let manifest = bob_ft.recv_folder_manifest().await.unwrap();

        for entry in &manifest.files {
            let (path, size) = bob_ft.recv_folder_file_header().await.unwrap();
            assert_eq!(path, entry.relative_path);
            assert_eq!(size, entry.size);
            bob_ft.send(b"FOLDER_FILE_ACK\n").await.unwrap();
            let out_path = std::path::Path::new(&download_path_clone).join(&path);
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            bob_ft
                .recv_file(&out_path.to_string_lossy(), size as i64, |_, _| {}, None)
                .await
                .unwrap();
        }

        let mut done_buf = [0u8; 32];
        bob_ft.recv(&mut done_buf).await.unwrap();
        manifest
    });

    let sender_stats = tcp_handle.await.unwrap();
    let recv_manifest = receiver_handle.await.unwrap();

    // ─── Verify ────────────────────────────────────────────────
    assert_eq!(sender_stats.total_files, 100);
    assert_eq!(sender_stats.completed_files, 100);
    assert_eq!(recv_manifest.total_files, 100);

    verify_directory_contents(&sender_path, &download_path, &recv_manifest);

    // Spot-check file count in subdirectories
    let group_a_count = std::fs::read_dir(download_dir.join("group_a"))
        .unwrap()
        .count();
    let group_b_count = std::fs::read_dir(download_dir.join("group_b"))
        .unwrap()
        .count();
    assert_eq!(group_a_count, 50, "group_a must have 50 files");
    assert_eq!(group_b_count, 50, "group_b must have 50 files");

    println!(
        "✅ 100-file batch transfer: {} files, {} bytes verified",
        recv_manifest.total_files, recv_manifest.total_bytes
    );

    // Cleanup
    let _ = std::fs::remove_dir_all(&sender_dir);
    let _ = std::fs::remove_dir_all(&download_dir);
}

#[tokio::test]
async fn test_folder_transfer_e2e_special_chars() {
    // ─── Setup: create files with special chars in names ────────
    let (sender_path, sender_dir) = create_temp_folder();
    let (download_path, download_dir) = create_download_dir();

    // Files with spaces, unicode, emoji, and colons
    std::fs::write(sender_dir.join("hello world.txt"), b"spaces in name").unwrap();
    std::fs::create_dir_all(sender_dir.join("sub dir")).unwrap();
    std::fs::write(sender_dir.join("sub dir/中文.txt"), b"chinese chars").unwrap();
    std::fs::write(sender_dir.join("emoji_😊🎉.txt"), b"emoji in name").unwrap();

    let transfer_id: u64 = 79004;
    let manifest = build_folder_manifest(&sender_path, transfer_id)
        .expect("manifest must build");
    assert_eq!(manifest.total_files, 3);

    // ─── UDP: folder notification ──────────────────────────────
    let alice = Arc::new(TestPeer::new("Alice", 14107).await);
    let bob = Arc::new(TestPeer::new("Bob", 14108).await);

    let (alice_tx, _alice_rx) = mpsc::unbounded_channel::<NetworkEvent>();
    let (bob_tx, mut bob_rx) = mpsc::unbounded_channel::<NetworkEvent>();

    let _a = alice.clone().spawn_recv(alice_tx);
    let _b = bob.clone().spawn_recv(bob_tx);
    sleep(Duration::from_millis(200)).await;

    let packet = build_folder_notification(
        &alice,
        transfer_id,
        &sender_path,
        &manifest.folder_name,
        manifest.total_files,
        manifest.total_bytes,
    );
    alice.send_to("127.0.0.1", bob.port, &packet).await;

    let event = tokio::time::timeout(Duration::from_secs(2), bob_rx.recv())
        .await
        .expect("Timeout: Bob did not receive folder notification")
        .expect("Bob channel closed");

    match event {
        NetworkEvent::Message(post) => {
            assert_eq!(post.contents.len(), 1);
            let fc = match &post.contents[0] {
                Content::File(fc) => fc,
                other => panic!("expected Content::File, got {:?}", other.content_type()),
            };
            assert_eq!(fc.file_type, IPMSG_FILE_DIR);
            assert!(fc.filename.starts_with("__FOLDER__"));
        }
        other => panic!("expected Message event, got {:?}", other),
    }

    println!("✅ UDP notification: special chars folder");

    // ─── TCP: folder data transfer ─────────────────────────────
    let tcp_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let tcp_addr = tcp_listener.local_addr().unwrap();

    let sender_path_clone = sender_path.clone();
    let tcp_handle = tokio::spawn(async move {
        let (stream, _) = tcp_listener.accept().await.expect("TCP accept");
        run_folder_sender(stream, &sender_path_clone, transfer_id)
            .await
            .expect("folder sender failed")
    });

    let download_path_clone = download_path.clone();
    let receiver_handle = tokio::spawn(async move {
        let mut bob_ft =
            FileTransfer::connect("127.0.0.1", tcp_addr.port())
                .await
                .expect("Bob TCP connect");
        let request = format!("FOLDER_MANIFEST_REQUEST\n{transfer_id}\n");
        bob_ft.send(request.as_bytes()).await.unwrap();
        let manifest = bob_ft.recv_folder_manifest().await.unwrap();

        for entry in &manifest.files {
            let (path, size) = bob_ft.recv_folder_file_header().await.unwrap();
            assert_eq!(path, entry.relative_path);
            assert_eq!(size, entry.size);
            bob_ft.send(b"FOLDER_FILE_ACK\n").await.unwrap();
            let out_path = std::path::Path::new(&download_path_clone).join(&path);
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            bob_ft
                .recv_file(&out_path.to_string_lossy(), size as i64, |_, _| {}, None)
                .await
                .unwrap();
        }

        let mut done_buf = [0u8; 32];
        bob_ft.recv(&mut done_buf).await.unwrap();
        manifest
    });

    let sender_stats = tcp_handle.await.unwrap();
    let recv_manifest = receiver_handle.await.unwrap();

    // ─── Verify ────────────────────────────────────────────────
    assert_eq!(sender_stats.total_files, 3);
    assert_eq!(recv_manifest.total_files, 3);

    // Verify special characters survive roundtrip
    assert!(
        recv_manifest.files.iter().any(|f| f.relative_path == "hello world.txt"),
        "spaces in filename must survive"
    );
    assert!(
        recv_manifest.files.iter().any(|f| f.relative_path == "sub dir/中文.txt"),
        "Chinese chars in filename must survive"
    );
    assert!(
        recv_manifest.files.iter().any(|f| f.relative_path.starts_with("emoji_")),
        "emoji in filename must survive"
    );

    verify_directory_contents(&sender_path, &download_path, &recv_manifest);

    println!(
        "✅ Special chars folder transfer: {} files verified",
        recv_manifest.total_files
    );

    // Cleanup
    let _ = std::fs::remove_dir_all(&sender_dir);
    let _ = std::fs::remove_dir_all(&download_dir);
}

#[tokio::test]
async fn test_folder_transfer_e2e_empty_folder_rejected() {
    // An empty folder must be rejected — build_folder_manifest returns None
    // and the engine refuses to initiate folder transfer for empty folders.
    let (sender_path, sender_dir) = create_temp_folder();

    let manifest = build_folder_manifest(&sender_path, 79005);
    assert!(
        manifest.is_none(),
        "empty folder must return None from build_folder_manifest"
    );

    // Also verify that the engine-level send_folder_to would fail
    // (we test this at the manifest level since we don't have a full engine)
    let non_empty = build_folder_manifest(
        std::env!("CARGO_MANIFEST_DIR"),
        79006,
    );
    assert!(
        non_empty.is_some(),
        "non-empty directory (crate root) must return Some"
    );

    let _ = std::fs::remove_dir_all(&sender_dir);
}

#[tokio::test]
async fn test_folder_transfer_e2e_nonexistent_path_rejected() {
    let manifest =
        build_folder_manifest("/tmp/feiq_nonexistent_folder_that_does_not_exist_xyz789", 79007);
    assert!(
        manifest.is_none(),
        "nonexistent path must return None from build_folder_manifest"
    );
}

//! FeiqEngine — the main Controller (MVC pattern).
//! Coordinates network, model, and storage. Dispatches events to frontend.
//! Mirrors feiqengine.cpp.

use crate::engine::events::FrontendEvent;
use crate::engine::tasks::FileTaskHandle;
use crate::model::contacts::ContactBook;
use crate::network::crypto::{
    compute_shared_secret, compute_shared_secret_from_raw, create_decryptor, create_encryptor,
    decrypt, encrypt, generate_keypair, FeiqDecryptor, FeiqEncryptor, is_feiq_plus_plus,
};
use crate::network::manager::NetworkManager;
use crate::network::relay::RelayClient;
use crate::network::NetworkEvent;
use crate::protocol::constants::*;
use crate::protocol::encoding::*;
use crate::protocol::serializer::*;
use crate::protocol::types::*;
use crate::storage::history::HistoryDb;
use crate::storage::settings::{AppConfig, ConnectionMode};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;

/// Global atomic counter for generating file task IDs across dispatch tasks.
static NEXT_FILE_TASK_ID: AtomicU64 = AtomicU64::new(1);

/// Unique packet ID generator
struct PacketIdGen(u64);

impl PacketIdGen {
    fn new() -> Self {
        Self(0)
    }
    fn next(&mut self) -> u64 {
        self.0 += 1;
        if self.0 >= u64::MAX {
            self.0 = 1;
        }
        self.0
    }
}

/// Unique file ID generator
struct FileIdGen(u64);

impl FileIdGen {
    fn new() -> Self {
        Self(0)
    }
    fn next(&mut self) -> u64 {
        self.0 += 1;
        if self.0 >= u64::MAX {
            self.0 = 1;
        }
        self.0
    }
}

/// The main engine controller
pub struct Engine {
    config: AppConfig,
    contacts: Arc<Mutex<ContactBook>>,
    network: Option<Arc<NetworkManager>>,
    relay_client: Option<Arc<RelayClient>>,
    event_tx: mpsc::UnboundedSender<FrontendEvent>,
    packet_id: PacketIdGen,
    file_id: FileIdGen,
    version: String,
    /// File transfer tasks (upload + download), keyed by local task ID
    file_tasks: Arc<std::sync::Mutex<HashMap<u64, Arc<FileTaskHandle>>>>,
    /// Chat history database (SQLite)
    history: Option<Arc<std::sync::Mutex<HistoryDb>>>,
    /// Signals periodic broadcast task to stop
    shutdown: Arc<AtomicBool>,
    started: bool,
    /// Per-peer crypto sessions (IP -> encryptor/decryptor) for ECDH+AES-256-GCM
    crypto_sessions: Arc<Mutex<HashMap<String, (FeiqEncryptor, FeiqDecryptor)>>>,
    /// Our current keypair for BR_ENTRY/ANSENTRY broadcast public key
    /// Stores raw [u8; 32] private key bytes (clonable, for multi-peer ECDH)
    /// rather than ring::agreement::EphemeralPrivateKey (single-use, not Clone).
    our_keypair: Arc<Mutex<Option<([u8; 32], Vec<u8>)>>>,
}

impl Engine {
    /// Create a new engine (does not start networking yet)
    /// `history_db_path` — optional path to SQLite history DB file
    pub fn new(
        config: AppConfig,
        event_tx: mpsc::UnboundedSender<FrontendEvent>,
        history_db_path: Option<PathBuf>,
    ) -> Self {
        // Build version string: "feiq_plus_plus#128#MAC#0#0#0#1#9"
        let mac = mac_address::get_mac_address()
            .ok()
            .flatten()
            .map(|ma| ma.to_string().replace(':', ""))
            .unwrap_or_default();
        let version = format!("feiq_plus_plus#128#{mac}#0#0#0#1#9");

        let history = history_db_path
            .and_then(|p| {
                HistoryDb::open(&p)
                    .map_err(|e| tracing::warn!("Failed to open history DB at {:?}: {}", p, e))
                    .ok()
            })
            .map(|db| Arc::new(std::sync::Mutex::new(db)));

        if history.is_some() {
            tracing::info!("Chat history DB opened");
        }

        Self {
            config,
            contacts: Arc::new(Mutex::new(ContactBook::new())),
            network: None,
            relay_client: None,
            event_tx,
            packet_id: PacketIdGen::new(),
            file_id: FileIdGen::new(),
            version,
            file_tasks: Arc::new(std::sync::Mutex::new(HashMap::new())),
            history,
            shutdown: Arc::new(AtomicBool::new(false)),
            started: false,
            crypto_sessions: Arc::new(Mutex::new(HashMap::new())),
            our_keypair: Arc::new(Mutex::new(None)),
        }
    }

    /// Get the version string used in protocol headers
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Start the engine: bind UDP, optionally connect relay, broadcast presence.
    pub async fn start(&mut self) -> anyhow::Result<()> {
        if self.started {
            anyhow::bail!("Engine already started");
        }
        self.shutdown.store(false, Ordering::Relaxed);

        let mode = self.config.mode.clone();

        // ─── UDP layer (skip if RelayOnly) ────────────────────
        if mode != ConnectionMode::RelayOnly {
            self.start_udp().await?;
        }

        // ─── Relay layer (skip if LanOnly or not configured) ──
        if mode != ConnectionMode::LanOnly
            && !self.config.relay_server_url.is_empty()
        {
            self.start_relay().await?;
        }

        // Load persisted contact meta (alias, signature, group_name)
        if let Err(e) = self.load_contact_meta() {
            tracing::warn!("Failed to load contact meta from DB: {e}");
        }

        self.started = true;
        tracing::info!(
            "Engine started: name={}, mode={mode:?}, version={}",
            self.config.name,
            self.version,
        );
        Ok(())
    }

    /// Start the UDP LAN transport
    async fn start_udp(&mut self) -> anyhow::Result<()> {
        let (net_tx, mut net_rx) = mpsc::unbounded_channel::<NetworkEvent>();

        let network = NetworkManager::new(net_tx, self.config.name.clone(), self.config.port).await?;
        let self_mac = network.self_mac().to_string();

        // Update version with actual MAC
        self.version = format!("feiq_plus_plus#128#{self_mac}#0#0#0#1#9");

        // Ensure keypair and include public key in BR_ENTRY broadcast
        let our_pub_key = self.ensure_keypair()?;
        let online_data = build_br_entry_ext(&self.config.name, &self.config.host, &self.version, Some(&our_pub_key));
        network.broadcast(&online_data).await?;

        if self.config.port != 2425 {
            network.broadcast_to_port(2425, &online_data).await?;
        }
        for ip in &self.config.custom_ips {
            let _ = network.send_to(ip, 2425, &online_data).await;
        }

        let network = Arc::new(network);

        // UDP receive loop
        let n = network.clone();
        tokio::spawn(async move {
            if let Err(e) = n.run().await {
                tracing::error!("Network recv loop exited: {e}");
            }
        });

        self.network = Some(network.clone());

        // Clone for event dispatch and periodic rebroadcast
        let network_for_dispatch = network.clone();
        let network_for_rebroadcast = network.clone();

        // Event dispatch with crypto key exchange
        let event_tx = self.event_tx.clone();
        let contacts = self.contacts.clone();
        let config = self.config.clone();
        let history_udp = self.history.clone();
        let file_tasks_udp = self.file_tasks().clone();
        let crypto_sessions = self.crypto_sessions.clone();
        let our_keypair = self.our_keypair.clone();
        let self_name = self.config.name.clone();
        let self_host = self.config.host.clone();
        let self_version = self.version.clone();

        tokio::spawn(async move {
            while let Some(event) = net_rx.recv().await {
                // ─── Pre-process crypto (key exchange) before handle_network_event ──
                let mut ans_pub_key: Option<Vec<u8>> = None;

                match &event {
                    NetworkEvent::FellowOnline(post) => {
                        if post.from.version.starts_with("feiq_plus_plus") && !post.from.public_key.is_empty() {
                            let peer_ip = &post.from.ip;
                            if !crypto_sessions.lock().unwrap().contains_key(peer_ip) {
                                if let Ok((our_priv, our_pub)) = generate_keypair() {
                                    if let Ok(secret) = compute_shared_secret(our_priv, &post.from.public_key) {
                                        let enc = create_encryptor(&secret);
                                        let dec = create_decryptor(&secret);
                                        crypto_sessions.lock().unwrap().insert(peer_ip.clone(), (enc, dec));
                                        ans_pub_key = Some(our_pub);
                                    }
                                }
                            }
                        }
                    }
                    NetworkEvent::FellowAnsEntry(post) => {
                        if post.from.version.starts_with("feiq_plus_plus") && !post.from.public_key.is_empty() {
                            let peer_ip = &post.from.ip;
                            if !crypto_sessions.lock().unwrap().contains_key(peer_ip) {
                                // Read our broadcast private key WITHOUT taking/consuming it.
                                // We store raw [u8; 32] bytes which are Copy, so we can clone
                                // the private key and use it for ECDH with MULTIPLE peers
                                // without rotating the broadcast keypair.
                                let kp = our_keypair.lock().unwrap();
                                if let Some((ref priv_key, _)) = *kp {
                                    if let Ok(secret) = compute_shared_secret_from_raw(priv_key, &post.from.public_key) {
                                        drop(kp);
                                        let enc = create_encryptor(&secret);
                                        let dec = create_decryptor(&secret);
                                        crypto_sessions.lock().unwrap().insert(peer_ip.clone(), (enc, dec));
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }

	                // ─── File share: handle GETDIRFILES (directory listing) ──
	                let is_get_dir_files = match &event {
	                    NetworkEvent::GetFileData { offset, file_id, .. } => {
	                        *offset > 0 && *file_id == 0 && !config.shared_dir.is_empty()
	                    }
	                    _ => false,
	                };

	                if is_get_dir_files {
	                    if let NetworkEvent::GetFileData {
	                        packet_no,
	                        offset: _,
	                        file_id: _,
	                        ref from,
	                    } = &event
	                    {
	                        let files =
	                            crate::network::tcp::list_directory(&config.shared_dir, &config.shared_dir);
	                        if !files.is_empty() {
	                            let files: Vec<FileContent> = files
	                                .into_iter()
	                                .enumerate()
	                                .map(|(i, mut f)| {
	                                    f.file_id = (i + 1) as u64;
	                                    f
	                                })
	                                .collect();
	                            let data = build_directory_listing(
	                                *packet_no,
	                                &self_name,
	                                &self_host,
	                                &self_version,
	                                &files,
	                            );
	                            let _ = network_for_dispatch.send_to(&from.ip, from.port, &data).await;
	                        }
	                    }
	                    continue;
	                }

	                let (reply, recv_reply) = handle_network_event(
                    event,
                    &event_tx,
                    &contacts,
                    &config,
                    &self_version,
                    &history_udp,
                    Some(&file_tasks_udp),
                    Some(&crypto_sessions),
                );

                // Send RECVMSG before ANSENTRY reply
                if let Some((recv_ip, recv_port, recv_data)) = recv_reply {
                    let _ = network_for_dispatch.send_to(&recv_ip, recv_port, &recv_data).await;
                }

                if let Some((ip, port)) = reply {
                    let ans_data = if let Some(ref pk) = ans_pub_key {
                        build_ans_entry_ext(&self_name, &self_host, &self_version, Some(pk))
                    } else {
                        build_ans_entry(&self_name, &self_host, &self_version)
                    };
                    let _ = network_for_dispatch.send_to(&ip, port, &ans_data).await;
                }
            }
        });

        // Periodic rebroadcast (includes pubkey in BR_ENTRY)
        let network_p = network_for_rebroadcast;
        let name_p = self.config.name.clone();
        let host_p = self.config.host.clone();
        let port_p = self.config.port;
        let custom_ips_p = self.config.custom_ips.clone();
        let version_p = self.version.clone();
        let shutdown_p = self.shutdown.clone();
        let our_keypair_rb = self.our_keypair.clone();

        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(30)).await;
            loop {
                if shutdown_p.load(Ordering::Relaxed) {
                    break;
                }
                let data = {
                    let kp = our_keypair_rb.lock().unwrap();
                    let pub_key = kp.as_ref().map(|(_, pk)| pk.as_slice());
                    build_br_entry_ext(&name_p, &host_p, &version_p, pub_key)
                };
                let _ = network_p.broadcast(&data).await;
                if port_p != 2425 {
                    let _ = network_p.broadcast_to_port(2425, &data).await;
                }
                for ip in &custom_ips_p {
                    let _ = network_p.send_to(ip, 2425, &data).await;
                }
                tracing::trace!("Periodic BR_ENTRY rebroadcast sent");
                tokio::time::sleep(Duration::from_secs(10)).await;
            }
            tracing::debug!("Periodic broadcast task shutting down");
        });

        Ok(())
    }

    /// Start the relay client transport
    async fn start_relay(&mut self) -> anyhow::Result<()> {
        let (relay_tx, mut relay_rx) = mpsc::unbounded_channel::<NetworkEvent>();

        let relay = RelayClient::new(
            &self.config.relay_server_url,
            &self.config.relay_room,
            &self.config.name,
            &self.config.host,
            &self.version,
            relay_tx,
        );

        let relay = Arc::new(relay);

        // Relay receive loop
        let r = relay.clone();
        tokio::spawn(async move {
            if let Err(e) = r.run().await {
                tracing::error!("Relay recv loop exited: {e}");
            }
        });

        self.relay_client = Some(relay.clone());

        // Relay event dispatch loop
        let event_tx = self.event_tx.clone();
        let contacts = self.contacts.clone();
        let config = self.config.clone();
        let history_relay = self.history.clone();
        let file_tasks_relay = self.file_tasks().clone();
        let version_relay = self.version.clone();

        tokio::spawn(async move {
            while let Some(event) = relay_rx.recv().await {
                let (_ans_reply, _recv_reply) = handle_network_event(
                    event,
                    &event_tx,
                    &contacts,
                    &config,
                    &version_relay,
                    &history_relay,
                    Some(&file_tasks_relay),
                    None,
                );
                // No ANSENTRY or RECVMSG reply for relay peers — server handles delivery
            }
        });

        tracing::info!(
            "Relay client started: server={}, room={}",
            self.config.relay_server_url,
            self.config.relay_room,
        );
        Ok(())
    }

    /// Stop the engine: broadcast BR_EXIT and cancel periodic rebroadcast
    pub async fn stop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);

        // Send offline broadcast so peers know we're leaving
        if let Some(ref network) = self.network {
            let exit_data = build_br_exit(&self.config.name, &self.config.host, &self.version);
            let _ = network.broadcast(&exit_data).await;
            // Signal the UDP receive loop to stop
            network.shutdown();
        }
        if let Some(ref relay) = self.relay_client {
            relay.shutdown();
        }

        self.started = false;
        self.network = None;
        self.relay_client = None;
        tracing::info!("Engine stopped");
    }

    /// Update config live (takes effect for new messages; periodic broadcast
    /// still uses the old name until restart)
    pub fn update_config(&mut self, config: AppConfig) {
        // Broadcast name change to peers if name actually changed
        if self.config.name != config.name && self.network.is_some() {
            // IPMSG_BR_ABSENCE signals a name/status change to peers
            self.config = config;
        } else {
            self.config = config;
        }
    }

    /// Get all known contacts (for frontend)
    pub fn get_contacts(&self) -> Vec<Fellow> {
        self.contacts.lock().unwrap().all()
    }

    /// Search contacts by query
    pub fn search_contacts(&self, query: &str) -> Vec<Fellow> {
        self.contacts.lock().unwrap().search(query)
    }

    /// Get a contact by IP
    pub fn find_contact(&self, ip: &str) -> Option<Fellow> {
        self.contacts.lock().unwrap().find_by_ip(ip)
    }

    /// Add a contact manually (user-added by IP)
    pub fn add_contact(&mut self, ip: &str) -> Fellow {
        let fellow = Fellow::new(ip);
        self.contacts.lock().unwrap().upsert(fellow.clone());
        fellow
    }

    /// Add a contact with custom port
    pub fn add_contact_with_port(&mut self, ip: &str, port: u16) -> Fellow {
        let mut fellow = Fellow::new(ip);
        fellow.port = port;
        self.contacts.lock().unwrap().upsert(fellow.clone());
        fellow
    }

    /// Internal: send text without saving to individual chat history.
    /// Used by `send_text_to_group` to avoid double-saving (N+1 copies).
    async fn send_text_raw(&self, ip: &str, port: u16, text: &str) -> anyhow::Result<()> {
        let text_gbk = encode_gbk(text);

        // Try to encrypt if crypto session exists for this peer
        let data = {
            let mut sessions = self.crypto_sessions.lock().unwrap();
            if let Some((ref mut enc, _)) = sessions.get_mut(ip) {
                let encrypted = encrypt(&text_gbk, enc)
                    .map_err(|_| anyhow::anyhow!("Encryption failed"))?;
                pack_message(
                    self.packet_id(),
                    &self.config.name,
                    &self.config.host,
                    &self.version,
                    IPMSG_SENDMSG | IPMSG_SENDCHECKOPT | IPMSG_ENCRYPTOPT,
                    &encrypted,
                )
            } else {
                build_text_message(
                    self.packet_id(),
                    &self.config.name,
                    &self.config.host,
                    &self.version,
                    text,
                )
            }
        };

        // Check if this peer is a relay peer — extract data before await
        let relay_peer_id = {
            self.contacts
                .lock()
                .unwrap()
                .find_by_ip(ip)
                .and_then(|f| match &f.source {
                    PeerSource::RelayPeer(id) => Some(id.clone()),
                    _ => None,
                })
        };

        let mut sent_ok = false;

        if let Some(peer_id) = relay_peer_id {
            if let Some(ref relay) = self.relay_client {
                tracing::info!("Engine sending text via relay to {peer_id}: {text}");
                let cmd = IPMSG_SENDMSG | IPMSG_SENDCHECKOPT;
                relay.send_to(&peer_id, cmd, &data).await?;
                sent_ok = true;
            }
        } else if let Some(ref network) = self.network {
            tracing::info!("Engine sending text via UDP to {ip}:{port}: {text}");
            network.send_to(ip, port, &data).await?;
            sent_ok = true;
        } else {
            tracing::warn!("Engine::send_text_to called but no transport available");
        }

        // ─── Enqueue for offline delivery ─────────────────
        if sent_ok {
            let is_offline = self
                .contacts.lock().unwrap()
                .find_by_ip(ip)
                .map(|f| !f.online)
                .unwrap_or(false);

            if is_offline {
                if let Some(ref history) = self.history {
                    let payload = serde_json::json!({ "text": text });
                    if let Err(e) = history.lock().unwrap().enqueue_pending(
                        ip,
                        "text",
                        &payload.to_string(),
                    ) {
                        tracing::warn!("Failed to enqueue offline message: {e}");
                    } else {
                        tracing::info!("Queued offline message for {ip}");
                    }
                }
            }
        }

        Ok(())
        }

    /// Send text message. Routes via relay if peer is from relay, else UDP.
    /// Automatically encrypts if crypto session exists for the peer.
    /// Automatically saves to chat history and enqueues offline messages.
    pub async fn send_text_to(&self, ip: &str, port: u16, text: &str) -> anyhow::Result<()> {
        self.send_text_raw(ip, port, text).await?;
// ─── Save to chat history ─────────────────────────
        if let Some(ref history) = self.history {
            let contact_name = self
                .contacts.lock().unwrap()
                .find_by_ip(ip)
                .map(|f| f.display_name().to_string())
                .unwrap_or_else(|| ip.to_string());
            let contents = vec![Content::Text {
                text: text.to_string(),
                format: String::new(),
            }];
            if let Err(e) = history.lock().unwrap().save_message(ip, &contact_name, 0, &contents) {
                tracing::warn!("Failed to save sent message to history: {e}");
            }
        }

                Ok(())
    }

    /// Send a sealed (read-and-destroy) text message with IPMSG_SECRETEXOPT.
    /// Routes via relay if peer is from relay, else UDP.
    /// Automatically encrypts if crypto session exists for the peer.
    pub async fn send_sealed_text_to(&self, ip: &str, port: u16, text: &str, _ttl: u32) -> anyhow::Result<()> {
        let text_gbk = encode_gbk(text);

        let data = {
            let mut sessions = self.crypto_sessions.lock().unwrap();
            if let Some((ref mut enc, _)) = sessions.get_mut(ip) {
                let encrypted = encrypt(&text_gbk, enc)
                    .map_err(|_| anyhow::anyhow!("Encryption failed"))?;
                pack_message(
                    self.packet_id(),
                    &self.config.name,
                    &self.config.host,
                    &self.version,
                    IPMSG_SENDMSG | IPMSG_SENDCHECKOPT | IPMSG_SECRETEXOPT | IPMSG_ENCRYPTOPT,
                    &encrypted,
                )
            } else {
                pack_message(
                    self.packet_id(),
                    &self.config.name,
                    &self.config.host,
                    &self.version,
                    IPMSG_SENDMSG | IPMSG_SENDCHECKOPT | IPMSG_SECRETEXOPT,
                    &text_gbk,
                )
            }
        };

        let relay_peer_id = {
            self.contacts
                .lock()
                .unwrap()
                .find_by_ip(ip)
                .and_then(|f| match &f.source {
                    PeerSource::RelayPeer(id) => Some(id.clone()),
                    _ => None,
                })
        };

        if let Some(peer_id) = relay_peer_id {
            if let Some(ref relay) = self.relay_client {
                return relay.send_to(
                    &peer_id,
                    IPMSG_SENDMSG | IPMSG_SENDCHECKOPT | IPMSG_SECRETEXOPT,
                    &data,
                ).await;
            }
        }

        if let Some(ref network) = self.network {
            network.send_to(ip, port, &data).await?;
        }

        Ok(())
    }

    /// Send IPMSG_READMSG notification for a sealed message that was read.
    /// This notifies the sender that their sealed message was consumed.
    pub async fn send_readmsg(&self, ip: &str, port: u16, packet_id: &str) -> anyhow::Result<()> {
        let data = build_readmsg(packet_id, &self.config.name, &self.config.host, &self.version);
        if let Some(ref network) = self.network {
            network.send_to(ip, port, &data).await?;
        }
        Ok(())
    }

    /// Send knock. Routes via relay if peer is from relay, else UDP.
    pub async fn send_knock_to(&self, ip: &str, port: u16) -> anyhow::Result<()> {
        let data = build_knock(&self.config.name, &self.config.host, &self.version);

        // Check relay peer — extract data before await
        let relay_peer_id = {
            self.contacts
                .lock()
                .unwrap()
                .find_by_ip(ip)
                .and_then(|f| match &f.source {
                    PeerSource::RelayPeer(id) => Some(id.clone()),
                    _ => None,
                })
        };

        if let Some(peer_id) = relay_peer_id {
            if let Some(ref relay) = self.relay_client {
                return relay.send_to(&peer_id, IPMSG_KNOCK, &data).await;
            }
        }

        // Default: UDP
        if let Some(ref network) = self.network {
            network.send_to(ip, port, &data).await?;
        }
        Ok(())
    }

    /// Get chat history for a contact (paginated, chronological order)
    pub fn get_chat_history(
        &self,
        contact_ip: &str,
        offset: i64,
        limit: i64,
    ) -> anyhow::Result<Vec<crate::storage::history::MessageRecord>> {
        match self.history {
            Some(ref history) => history.lock().unwrap().get_messages(contact_ip, offset, limit),
            None => Ok(Vec::new()),
        }
    }

    /// Set a contact's alias, persisting to DB and updating contact book.
    /// Sends a ContactUpdate event so the frontend refreshes.
    pub fn set_contact_alias(&self, ip: &str, alias: &str) -> anyhow::Result<()> {
        // Persist to DB
        if let Some(ref history) = self.history {
            history.lock().unwrap().set_contact_alias(ip, alias)?;
        }

        // Update contact book in-memory
        let mut book = self.contacts.lock().unwrap();
        if let Some(mut fellow) = book.find_by_ip(ip) {
            fellow.alias = alias.to_string();
            book.upsert(fellow.clone());
            // Drop the lock before sending event
            drop(book);
            let _ = self.event_tx.send(FrontendEvent::ContactUpdate { fellow });
        }
        Ok(())
    }

    /// Set a contact's group name (persisted to DB).
    pub fn set_contact_group(&self, ip: &str, group_name: &str) -> anyhow::Result<()> {
        if let Some(ref history) = self.history {
            history.lock().unwrap().set_contact_group(ip, group_name)?;
        }
        Ok(())
    }

    /// Create a new group with given name and member IPs, persisted to DB.
    /// Replaces any existing group with the same name.
    pub fn create_group(&self, name: &str, member_ips: &[String]) -> anyhow::Result<()> {
        match self.history {
            Some(ref history) => history.lock().unwrap().save_group(name, member_ips),
            None => anyhow::bail!("History database not available"),
        }
    }

    /// Get all groups from DB: Vec of (group_name, member_ips)
    pub fn get_groups(&self) -> anyhow::Result<Vec<(String, Vec<String>)>> {
        match self.history {
            Some(ref history) => history.lock().unwrap().get_groups(),
            None => Ok(Vec::new()),
        }
    }

    /// Load all contact meta from DB (alias, signature, group_name) and
    /// apply to the in-memory contact book. Does not trigger events —
    /// intended for restoration at startup.
    pub fn load_contact_meta(&self) -> anyhow::Result<()> {
        if let Some(ref history) = self.history {
            let meta_map = history.lock().unwrap().load_all_contact_meta()?;
            if meta_map.is_empty() {
                return Ok(());
            }
            let mut book = self.contacts.lock().unwrap();
            for (ip, (alias, signature, group_name)) in meta_map {
                if let Some(mut fellow) = book.find_by_ip(&ip) {
                    fellow.alias = alias;
                    fellow.signature = signature;
                    fellow.group_name = group_name;
                    book.upsert(fellow);
                }
            }
        }
        Ok(())
    }

    /// Export all chat history as a JSON string
    pub fn export_history(&self) -> anyhow::Result<String> {
        match self.history {
            Some(ref history) => {
                let value = history.lock().unwrap().export_all()?;
                Ok(value.to_string())
            }
            None => anyhow::bail!("History database not available"),
        }
    }

    /// Import messages from a JSON string. Returns the count of imported messages.
    pub fn import_history(&self, json: &str) -> anyhow::Result<usize> {
        match self.history {
            Some(ref history) => {
                let value: serde_json::Value = serde_json::from_str(json)?;
                let messages = value["messages"]
                    .as_array()
                    .ok_or_else(|| anyhow::anyhow!("Invalid JSON: missing 'messages' array"))?;
                let count = history.lock().unwrap().import_messages(messages)?;
                Ok(count)
            }
            None => anyhow::bail!("History database not available"),
        }
    }

    // ─── Blacklist ─────────────────────────────────────────────

    /// Check whether an IP is blacklisted
    pub fn is_ip_blacklisted(&self, ip: &str) -> bool {
        match self.history {
            Some(ref history) => history.lock().unwrap().is_blacklisted(ip).unwrap_or(true),
            None => false,
        }
    }

    /// Add an IP to the blacklist
    pub fn add_to_blacklist(&self, ip: &str) {
        if let Some(ref history) = self.history {
            if let Err(e) = history.lock().unwrap().add_to_blacklist(ip) {
                tracing::warn!("Failed to add {ip} to blacklist: {e}");
            }
        }
    }

    /// Remove an IP from the blacklist
    pub fn remove_from_blacklist(&self, ip: &str) {
        if let Some(ref history) = self.history {
            if let Err(e) = history.lock().unwrap().remove_from_blacklist(ip) {
                tracing::warn!("Failed to remove {ip} from blacklist: {e}");
            }
        }
    }

    /// Get all blacklisted IPs
    pub fn get_blacklist(&self) -> Vec<String> {
        match self.history {
            Some(ref history) => match history.lock().unwrap().get_blacklist() {
                Ok(list) => list,
                Err(e) => {
                    tracing::warn!("Failed to get blacklist from DB: {e}");
                    Vec::new()
                }
            },
            None => Vec::new(),
        }
    }

    /// Generate next packet ID
    fn packet_id(&self) -> u64 {
        // Simple: use timestamp millis as ID
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }

    // ─── Chat History Search ──────────────────────────────────

    /// Search chat history across all contacts by query string.
    pub fn search_chat_history(&self, query: &str, limit: i64) -> anyhow::Result<Vec<crate::storage::history::MessageRecord>> {
        match self.history {
            Some(ref history) => history.lock().unwrap().search_messages(query, limit),
            None => Ok(Vec::new()),
        }
    }

    // ─── File Task Management ─────────────────────────────────

    /// Register a file transfer task and return its local task ID.
    pub fn register_file_task(&self, task: FileTaskHandle) -> u64 {
        let task_id = task.snapshot().id;
        let mut tasks = self.file_tasks.lock().unwrap();
        tasks.insert(task_id, Arc::new(task));
        task_id
    }

    /// Get a file transfer task by local task ID.
    pub fn get_file_task(&self, task_id: u64) -> Option<Arc<FileTaskHandle>> {
        self.file_tasks.lock().unwrap().get(&task_id).cloned()
    }

    /// Cancel a file transfer task by local task ID.
    pub fn cancel_file_task(&self, task_id: u64) {
        if let Some(task) = self.file_tasks.lock().unwrap().get(&task_id) {
            task.request_cancel();
            task.set_canceled();
        }
    }

    /// Getter for history database reference (for handle_network_event parameter).
    pub fn history(&self) -> Option<&Arc<std::sync::Mutex<HistoryDb>>> {
        self.history.as_ref()
    }

    /// Getter for file_tasks Arc (for sharing across dispatch tasks).
    pub fn file_tasks(&self) -> &Arc<std::sync::Mutex<HashMap<u64, Arc<FileTaskHandle>>>> {
        &self.file_tasks
    }

    /// Get a reference to the network manager (for TCP file transfers).
    pub fn network(&self) -> Option<&Arc<NetworkManager>> {
        self.network.as_ref()
    }

    /// Get a reference to the event sender for emitting frontend events.
    pub fn event_tx(&self) -> &mpsc::UnboundedSender<FrontendEvent> {
        &self.event_tx
    }

    /// Get the next local task ID (monotonically increasing).
    fn next_task_id(&mut self) -> u64 {
        self.file_id.next()
    }

    // ─── Encryption Pipeline (Phase 5.1) ──────────────────────────

    /// Ensure we have a keypair for BR_ENTRY/ANSENTRY broadcasts.
    /// Returns our current public key (32 bytes).
    fn ensure_keypair(&self) -> anyhow::Result<Vec<u8>> {
        let mut kp = self.our_keypair.lock().unwrap();
        if let Some((_, pub_key)) = kp.as_ref() {
            return Ok(pub_key.clone());
        }
        let (priv_key, pub_key) = crate::network::crypto::generate_broadcast_keypair();
        *kp = Some((priv_key, pub_key.clone()));
        Ok(pub_key)
    }
}

// ─── File Transfer ─────────────────────────────────────────────
impl Engine {
    /// Send a file notification to a peer.
    /// Creates a FileTaskHandle (Upload type), registers it, and sends
    /// the file notification message via UDP or relay.
    /// Returns the local task ID for tracking the transfer.
    pub async fn send_file_to(&self, ip: &str, file_path: &str) -> anyhow::Result<u64> {
        let task_id = NEXT_FILE_TASK_ID.fetch_add(1, Ordering::Relaxed);
        let packet_no = self.packet_id();

        let mut fc = create_file_content(file_path)
            .ok_or_else(|| anyhow::anyhow!("File not found: {}", file_path))?;
        fc.file_id = packet_no;
        fc.packet_no = packet_no;
        fc.local_task_id = Some(task_id);

        // Look up peer info (port, relay peer ID, display name)
        let (port, relay_peer_id, fellow_name) = {
            let contacts = self.contacts.lock().unwrap();
            let fellow = contacts.find_by_ip(ip);
            let port = fellow.as_ref().map(|f| f.port).unwrap_or(2425);
            let relay_peer_id = fellow.as_ref().and_then(|f| match &f.source {
                PeerSource::RelayPeer(id) => Some(id.clone()),
                _ => None,
            });
            let fellow_name = fellow
                .map(|f| f.display_name().to_string())
                .unwrap_or_else(|| ip.to_string());
            (port, relay_peer_id, fellow_name)
        };

        // Create and register file task
        let task = FileTaskHandle::new(
            task_id,
            ip.to_string(),
            fellow_name,
            fc.clone(),
            FileTaskType::Upload,
        );
        self.register_file_task(task);

        // Build file notification message
        let data = build_file_message(
            packet_no,
            &self.config.name,
            &self.config.host,
            &self.version,
            &fc,
        );

        // Send via relay or UDP
        if let Some(peer_id) = relay_peer_id {
            if let Some(ref relay) = self.relay_client {
                relay
                    .send_to(&peer_id, IPMSG_SENDMSG | IPMSG_FILEATTACHOPT, &data)
                    .await?;
            } else {
                anyhow::bail!("Relay not connected");
            }
        } else if let Some(ref network) = self.network {
            network.send_to(ip, port, &data).await?;
        } else {
            anyhow::bail!("No transport available");
        }

        // Emit FileStateChanged event to frontend
        let _ = self.event_tx.send(FrontendEvent::FileStateChanged {
            task_id,
            state: FileTaskState::NotStart,
            message: format!("Sending file: {}", fc.filename),
        });

        Ok(task_id)
    }
}

// ─── File Sharing (Phase 5.3) ─────────────────────────────────
impl Engine {
    /// Handle a GETDIRFILES request from a peer for directory listing.
    /// Returns `Some(Vec<FileContent>)` with the directory listing,
    /// or `None` if no shared directory is configured.
    pub fn handle_file_share_request(&self, request: &GetFileData) -> Option<Vec<FileContent>> {
        if request.offset > 0 && request.file_id == 0 {
            let config = &self.config;
            if config.shared_dir.is_empty() {
                tracing::debug!("File share request ignored: no shared directory configured");
                return None;
            }
            let dir_path = &config.shared_dir;
            let mut files = crate::network::tcp::list_directory(dir_path, dir_path);
            if files.is_empty() {
                tracing::debug!("File share request: shared directory is empty");
                return None;
            }
            // Assign file IDs sequentially starting at 1 (0 means root)
            for (i, f) in files.iter_mut().enumerate() {
                f.file_id = (i + 1) as u64;
            }
            Some(files)
        } else {
            None
        }
    }
}

// ─── Network event handler (runs in tokio task) ──────────────

/// Check whether an IP is blacklisted (returns true on DB error to fail closed)
fn is_ip_blacklisted(ip: &str, history: &Option<Arc<std::sync::Mutex<HistoryDb>>>) -> bool {
    match history {
        Some(ref h) => h.lock().unwrap().is_blacklisted(ip).unwrap_or(true),
        None => false,
    }
}

fn handle_network_event(
    event: NetworkEvent,
    event_tx: &mpsc::UnboundedSender<FrontendEvent>,
    contacts: &Arc<Mutex<ContactBook>>,
    config: &AppConfig,
    version: &str,
    history: &Option<Arc<std::sync::Mutex<HistoryDb>>>,
    file_tasks: Option<&Arc<std::sync::Mutex<HashMap<u64, Arc<FileTaskHandle>>>>>,
    crypto_sessions: Option<&Arc<Mutex<HashMap<String, (FeiqEncryptor, FeiqDecryptor)>>>>,
) -> (Option<(String, u16)>, Option<(String, u16, Vec<u8>)>) {
    // ─── Blacklist check — drop events from blacklisted IPs ──
    let event_ip = match &event {
        NetworkEvent::FellowOnline(post)
        | NetworkEvent::FellowAnsEntry(post)
        | NetworkEvent::FellowOffline(post)
        | NetworkEvent::Message(post) => Some(&post.from.ip),
        NetworkEvent::GetFileData { from, .. } => Some(&from.ip),
        NetworkEvent::ReleaseFiles { from, .. } => Some(&from.ip),
        NetworkEvent::Error(_) => None,
    };
    if let Some(ip) = event_ip {
        if is_ip_blacklisted(ip, history) {
            tracing::debug!("Dropping event from blacklisted IP: {ip}");
            return (None, None);
        }
    }

    match event {
        NetworkEvent::FellowOnline(post) => {
            let ip = post.from.ip.clone();
            let port = post.from.port;
            let fellow = post.from;
            let mut book = contacts.lock().unwrap();
            let is_new = book.find(&fellow.ip, fellow.port).is_none();
            let changed = book.upsert(fellow.clone());

            let mut display_fellow = book.find(&fellow.ip, fellow.port)
                .unwrap_or(fellow);
            display_fellow.online = true;

            if is_new || changed {
                let _ = event_tx.send(FrontendEvent::ContactUpdate {
                    fellow: display_fellow,
                });
            }

            // ─── Drain & deliver offline messages ──────────
            if let Some(ref history) = history {
                let pending = match history.lock().unwrap().drain_pending(&ip) {
                    Ok(p) => p,
                    Err(e) => {
                        tracing::warn!("Failed to drain pending messages for {ip}: {e}");
                        Vec::new()
                    }
                };
                if !pending.is_empty() {
                    tracing::info!("Delivering {} offline messages for {}", pending.len(), ip);
                    for msg in &pending {
                        // Reconstruct Content from stored JSON payload
                        let contents = if msg.message_type == "text" {
                            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&msg.payload_json) {
                                vec![Content::Text {
                                    text: parsed["text"].as_str().unwrap_or(&msg.payload_json).to_string(),
                                    format: String::new(),
                                }]
                            } else {
                                vec![Content::Text {
                                    text: msg.payload_json.clone(),
                                    format: String::new(),
                                }]
                            }
                        } else {
                            vec![Content::Text {
                                text: format!("[离线消息: {}]", msg.message_type),
                                format: String::new(),
                            }]
                        };

                        let _ = event_tx.send(FrontendEvent::NewMessage {
                            from_ip: ip.clone(),
                            from_name: String::new(), // will be filled by contact lookup in frontend
                            contents,
                            timestamp: msg.created_at,
                        });
                    }
                }
            }

            // Reply ANSENTRY for mutual discovery (return to caller)
            (Some((ip, port)), None)
        }
        NetworkEvent::FellowAnsEntry(post) => {
            let fellow = post.from;
            let mut book = contacts.lock().unwrap();
            let is_new = book.find(&fellow.ip, fellow.port).is_none();
            let changed = book.upsert(fellow.clone());

            let mut display_fellow = book.find(&fellow.ip, fellow.port)
                .unwrap_or(fellow);
            display_fellow.online = true;

            if is_new || changed {
                let _ = event_tx.send(FrontendEvent::ContactUpdate {
                    fellow: display_fellow,
                });
            }
            (None, None) // ANSENTRY is itself a reply, don't reply to a reply
        }
        NetworkEvent::FellowOffline(post) => {
            let mut fellow = post.from;
            fellow.online = false;
            let mut book = contacts.lock().unwrap();
            book.upsert(fellow.clone());
            let _ = event_tx.send(FrontendEvent::ContactUpdate { fellow });
            (None, None)
        }
        NetworkEvent::Message(mut post) => {
            // ─── Decrypt if IPMSG_ENCRYPTOPT flag is set ──────────
            if is_opt_set(post.cmd_id, IPMSG_ENCRYPTOPT) {
                if let Some(sessions) = crypto_sessions {
                    let ip = post.from.ip.clone();
                    if let Some((_, dec)) = sessions.lock().unwrap().get_mut(&ip) {
                        if let Ok(plaintext) = decrypt(&post.extra, dec) {
                            post.extra = plaintext;
                            post.cmd_id &= !IPMSG_ENCRYPTOPT;
                            post.contents.clear();
                            let is_utf8 = is_opt_set(post.cmd_id, IPMSG_UTF8OPT);
                            let text = decode_by_utf8opt(&post.extra, is_utf8);
                            if !text.is_empty() {
                                post.contents.push(Content::Text {
                                    text,
                                    format: String::new(),
                                });
                            }
                        }
                    }
                }
            }

            // Filter unsupported content types (mirrors original onMsg)
            let mut reply_text = String::new();
            post.contents.retain(|c| {
                match c {
                    Content::File(fc) => {
                        if fc.file_type == IPMSG_FILE_DIR {
                            reply_text.push_str(
                                &format!("feiq++ does not support receiving folders: {}\n",
                                    fc.filename),
                            );
                            false
                        } else {
                            true
                        }
                    }
                    Content::Image { .. } => {
                        reply_text.push_str(
                            "feiq++ does not support inline images. Please send as file.\n",
                        );
                        false
                    }
                    Content::Text { text, .. } => {
                        // Filter feiq encoded messages
                        if starts_with(text, "/~#>") && ends_with(text, "<B~") {
                            false
                        } else {
                            true
                        }
                    }
                    Content::Id { .. } => false, // Don't show read receipts as messages
                    _ => true,
                }
            });

            // Save fields for RECVMSG before post.from might be moved
            let recv_sender_ip = post.from.ip.clone();
            let recv_sender_port = post.from.port;

            if !post.contents.is_empty() {
                let timestamp = post.when.timestamp_millis();
                let from_name = post.from.display_name().to_string();
                let from_ip = post.from.ip.clone();
                tracing::info!(
                    "Dispatching message from {} (ip={}): {} contents",
                    from_name,
                    from_ip,
                    post.contents.len(),
                );

                // ─── Process file contents: create FileTaskHandle entries ──
                if let Some(ref ft) = file_tasks {
                    for content in &mut post.contents {
                        if let Content::File(ref mut fc) = content {
                            if fc.file_type != IPMSG_FILE_DIR {
                                let task_id =
                                    NEXT_FILE_TASK_ID.fetch_add(1, Ordering::Relaxed);
                                fc.local_task_id = Some(task_id);
                                let task = FileTaskHandle::new(
                                    task_id,
                                    from_ip.clone(),
                                    from_name.clone(),
                                    fc.clone(),
                                    FileTaskType::Download,
                                );
                                ft.lock().unwrap().insert(task_id, Arc::new(task));
                                let _ = event_tx.send(FrontendEvent::FileStateChanged {
                                    task_id,
                                    state: FileTaskState::NotStart,
                                    message: format!("File received: {}", fc.filename),
                                });
                            }
                        }
                    }
                }

                                // ─── Group routing: check for [groupname] prefix in text ──
                let group_target = {
                    let mut result: Option<String> = None;
                    if let Some(ref h) = history {
                        for content in &post.contents {
                            if let Content::Text { text, .. } = content {
                                if let Some(rest) = text.strip_prefix('[') {
                                    if let Some(end) = rest.find(']') {
                                        let gname = &rest[..end];
                                        if let Ok(groups) = h.lock().unwrap_or_else(|e| e.into_inner()).get_groups() {
                                            if groups.iter().any(|(n, _)| n == gname) {
                                                result = Some(gname.to_string());
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    result
                };
                if let Some(ref gname) = group_target {
                    let prefix = format!("[{}]", gname);
                    for content in &mut post.contents {
                        if let Content::Text { ref mut text, .. } = content {
                            if let Some(rest) = text.strip_prefix(&prefix) {
                                *text = rest.trim().to_string();
                            }
                        }
                    }
                    let group_key = format!("group:{}", gname);
                    if let Some(ref history) = history {
                        if let Err(e) = history.lock().unwrap_or_else(|e| e.into_inner()).save_message(
                            &group_key, &from_name, 1, &post.contents,
                        ) { tracing::warn!("Failed to save group message to history: {e}"); }
                    }
                    let _ = event_tx.send(FrontendEvent::NewMessage {
                        from_ip: group_key, from_name,
                        contents: std::mem::take(&mut post.contents), timestamp,
                    });
                } else {
                    if let Some(ref history) = history {
                        if let Err(e) = history.lock().unwrap_or_else(|e| e.into_inner()).save_message(
                            &post.from.ip, &from_name, 1, &post.contents,
                        ) { tracing::warn!("Failed to save received message to history: {e}"); }
                    }
                    let _ = event_tx.send(FrontendEvent::NewMessage {
                        from_ip: post.from.ip.clone(), from_name: from_name.clone(),
                        contents: std::mem::take(&mut post.contents), timestamp,
                    });
                }
                // Update contact book
                let mut book = contacts.lock().unwrap();
                book.upsert(post.from);
            } else {
                // Contents are empty — still update contact on any message
                let mut book = contacts.lock().unwrap();
                book.upsert(post.from);
            }

            // ─── Auto-reply RECVMSG if SENDCHECKOPT is set ────────────
            if is_opt_set(post.cmd_id, IPMSG_SENDCHECKOPT) {
                let recv_data = build_recvmsg(
                    &post.packet_no,
                    &config.name,
                    &config.host,
                    version,
                );
                (None, Some((recv_sender_ip, recv_sender_port, recv_data)))
            } else {
                (None, None)
            }
        }
        NetworkEvent::GetFileData {
            packet_no,
            file_id,
            offset,
            from,
        } => {
            // File data request — handled in the engine's event loop
            // where async TCP accept is available. Here we just log it.
            tracing::info!(
                "GetFileData received from {}: packet_no={}, file_id={}, offset={}",
                from.ip,
                packet_no,
                file_id,
                offset,
            );
            (None, None)
        }
        NetworkEvent::ReleaseFiles {
            packet_no,
            file_id,
            from,
        } => {
            tracing::info!(
                "ReleaseFiles from {}: packet_no={}, file_id={}",
                from.ip, packet_no, file_id,
            );
            // Find and cancel matching file tasks
            if let Some(ref ft) = file_tasks {
                let tasks = ft.lock().unwrap();
                for (_task_id, task) in tasks.iter() {
                    let snap = task.snapshot();
                    if snap.content.packet_no == packet_no
                        && snap.content.file_id == file_id
                    {
                        task.request_cancel();
                        task.set_canceled();
                        let _ = event_tx.send(FrontendEvent::FileStateChanged {
                            task_id: snap.id,
                            state: FileTaskState::Canceled,
                            message: format!(
                                "Peer released file: {}",
                                snap.content.filename
                            ),
                        });
                    }
                }
            }
            (None, None)
        }
        NetworkEvent::Error(msg) => {
            let _ = event_tx.send(FrontendEvent::Error(msg));
            (None, None)
        }
    }
}

// ─── Protocol message builders ───────────────────────────────

/// Build IPMSG_BR_ENTRY broadcast message (without public key)
pub fn build_br_entry(name: &str, host: &str, version: &str) -> Vec<u8> {
    build_br_entry_ext(name, host, version, None)
}

/// Build BR_ENTRY with optional pub key appended: [GBK name][NUL][32 pubkey bytes]
pub(crate) fn build_br_entry_ext(name: &str, host: &str, version: &str, pubkey: Option<&[u8]>) -> Vec<u8> {
    let mut body = encode_gbk(name);
    if let Some(pk) = pubkey {
        body.push(0x00);
        body.extend_from_slice(pk);
    }
    pack_message(0, name, host, version, IPMSG_BR_ENTRY, &body)
}

/// Build IPMSG_BR_EXIT broadcast message
pub fn build_br_exit(name: &str, host: &str, version: &str) -> Vec<u8> {
    let name_gbk = encode_gbk(name);
    pack_message(
        0,
        name,
        host,
        version,
        IPMSG_BR_EXIT,
        &name_gbk,
    )
}

/// Build IPMSG_ANSENTRY reply (without public key)
pub fn build_ans_entry(name: &str, host: &str, version: &str) -> Vec<u8> {
    build_ans_entry_ext(name, host, version, None)
}

/// Build ANSENTRY with optional pub key appended: [GBK name][NUL][32 pubkey bytes]
pub(crate) fn build_ans_entry_ext(name: &str, host: &str, version: &str, pubkey: Option<&[u8]>) -> Vec<u8> {
    let mut body = encode_gbk(name);
    if let Some(pk) = pubkey {
        body.push(0x00);
        body.extend_from_slice(pk);
    }
    pack_message(0, name, host, version, IPMSG_ANSENTRY, &body)
}

/// Build IPMSG_SENDMSG text message
pub fn build_text_message(
    packet_no: u64,
    name: &str,
    host: &str,
    version: &str,
    text: &str,
) -> Vec<u8> {
    let text_gbk = encode_gbk(text);
    pack_message(
        packet_no,
        name,
        host,
        version,
        IPMSG_SENDMSG | IPMSG_SENDCHECKOPT,
        &text_gbk,
    )
}

/// Build IPMSG_RECVMSG confirmation
pub fn build_recvmsg(packet_no: &str, name: &str, host: &str, version: &str) -> Vec<u8> {
    let payload = packet_no.as_bytes().to_vec();
    pack_message(
        0,
        name,
        host,
        version,
        IPMSG_RECVMSG,
        &payload,
    )
}

/// Build IPMSG_READMSG (sealed message read notification)
pub fn build_readmsg(packet_no: &str, name: &str, host: &str, version: &str) -> Vec<u8> {
    let payload = packet_no.as_bytes().to_vec();
    pack_message(
        0,
        name,
        host,
        version,
        IPMSG_READMSG,
        &payload,
    )
}

/// Build IPMSG_KNOCK (window shake)
pub fn build_knock(name: &str, host: &str, version: &str) -> Vec<u8> {
    pack_message(
        0,
        name,
        host,
        version,
        IPMSG_KNOCK,
        &[],
    )
}

/// Build IPMSG_SENDMSG | IPMSG_FILEATTACHOPT file notification
/// Format: \0fileId:filename:size:modifyTime:fileType:\x07
pub fn build_file_message(
    packet_no: u64,
    name: &str,
    host: &str,
    version: &str,
    content: &FileContent,
) -> Vec<u8> {
    let mut body = vec![MSG_NULL]; // starts with null byte (no text)
    let filename_gbk = encode_gbk(&content.filename.replace(':', "::"));

    write!(&mut body, "{}:", content.file_id).unwrap();
    body.extend_from_slice(&filename_gbk);
    write!(
        &mut body,
        ":{:X}:{:X}:{:X}:",
        content.size, content.modify_time, content.file_type
    )
    .unwrap();
    body.push(FILELIST_SEPARATOR);

    pack_message(
        packet_no,
        name,
        host,
        version,
        IPMSG_SENDMSG | IPMSG_FILEATTACHOPT,
        &body,
    )
}

/// Build a directory listing response (multiple file entries) for IPMSG_GETDIRFILES.
/// Format (per file): id:filename:size:modifyTime:fileType:\x07
pub fn build_directory_listing(
    packet_no: u64,
    name: &str,
    host: &str,
    version: &str,
    files: &[FileContent],
) -> Vec<u8> {
    let mut body = vec![MSG_NULL]; // starts with null byte (no text)

    for content in files {
        let filename_gbk = encode_gbk(&content.filename.replace(':', "::"));

        write!(&mut body, "{}:", content.file_id).unwrap();
        body.extend_from_slice(&filename_gbk);
        write!(
            &mut body,
            ":{:X}:{:X}:{:X}:",
            content.size, content.modify_time, content.file_type
        )
        .unwrap();
        body.push(FILELIST_SEPARATOR);
    }

    pack_message(
        packet_no,
        name,
        host,
        version,
        IPMSG_SENDMSG | IPMSG_FILEATTACHOPT,
        &body,
    )
}

/// Build IPMSG_GETFILEDATA TCP request
/// Format: packetNo:fileId:offset:
pub fn build_get_file_data(packet_no: u64, file_id: u64, offset: i64) -> Vec<u8> {
    let mut data = Vec::new();
    write!(&mut data, "{}:{}:{}:", packet_no, file_id, offset).unwrap();
    pack_message(
        packet_no,
        "",
        "",
        "",
        if offset == 0 {
            IPMSG_GETFILEDATA
        } else {
            IPMSG_GETDIRFILES
        },
        &data,
    )
}

/// Create a FileContent from a local file path
pub fn create_file_content(file_path: &str) -> Option<FileContent> {
    let meta = std::fs::metadata(file_path).ok()?;
    let filename = get_filename_from_path(file_path);

    Some(FileContent {
        file_id: 0, // filled by caller
        filename,
        path: file_path.to_string(),
        size: meta.len() as i64,
        modify_time: meta
            .modified()
            .ok()?
            .duration_since(std::time::UNIX_EPOCH)
            .ok()?
            .as_secs() as i64,
        file_type: if meta.is_dir() {
            IPMSG_FILE_DIR
        } else {
            IPMSG_FILE_REGULAR
        },
        packet_no: 0,
        local_task_id: None,
    })
}

use std::io::Write as IoWrite;

// ─── Group Chat ─────────────────────────────────────────────────

impl Engine {
    /// Send text to all members of a group (P2P dispatch).
    /// The message is prefixed with `[group_name]` so each recipient
    /// knows which group it came from (they forward to all members locally).
    /// Also saves to local history under the synthetic key `group:{group_name}`.
    pub async fn send_text_to_group(&self, group_name: &str, text: &str) -> anyhow::Result<()> {
        let groups = match self.history {
            Some(ref history) => history.lock().unwrap().get_groups()?,
            None => anyhow::bail!("History not available"),
        };
        let members = groups
            .into_iter()
            .find(|(name, _)| name == group_name)
            .map(|(_, members)| members)
            .ok_or_else(|| anyhow::anyhow!("Group '{}' not found", group_name))?;

        let prefixed = format!("[{}] {}", group_name, text);
        for ip in &members {
            // Use default port 2425 for LAN, or look up the contact's actual port
            let port = self
                .contacts
                .lock()
                .unwrap()
                .find_by_ip(ip)
                .map(|f| f.port)
                .unwrap_or(2425);
            if let Err(e) = self.send_text_raw(ip, port, &prefixed).await {
                tracing::warn!("Failed to send to group member {ip}: {e}");
            }
        }

        // Also save to local history under group key
        if let Some(ref history) = self.history {
            let contents = vec![Content::Text {
                text: prefixed.clone(),
                format: String::new(),
            }];
            let group_key = format!("group:{}", group_name);
            let _ = history
                .lock()
                .unwrap()
                .save_message(&group_key, group_name, 0, &contents);
        }
        Ok(())
    }
}



impl Drop for Engine {
    fn drop(&mut self) {
        if !self.shutdown.load(Ordering::Relaxed) {
            if let Some(ref network) = self.network {
                let network = network.clone();
                let name = self.config.name.clone();
                let host = self.config.host.clone();
                let version = self.version.clone();
                if let Ok(handle) = tokio::runtime::Handle::try_current() {
                    handle.spawn(async move {
                        let exit_data = build_br_exit(&name, &host, &version);
                        let _ = network.broadcast(&exit_data).await;
                    });
                }
            }
            if let Some(ref relay) = self.relay_client {
                relay.shutdown();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::settings::AppConfig;

    #[test]
    fn test_create_group_no_history() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let engine = Engine::new(AppConfig::default(), tx, None);
        let result = engine.create_group("Test", &["10.0.0.1".into()]);
        assert!(result.is_err());
        assert!(
            result.unwrap_err().to_string().contains("History"),
            "Expected error mentioning History"
        );
    }

    #[test]
    fn test_get_groups_no_history() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let engine = Engine::new(AppConfig::default(), tx, None);
        let groups = engine.get_groups().unwrap();
        assert!(groups.is_empty());
    }

    #[tokio::test]
    async fn test_send_text_to_group_no_history() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let engine = Engine::new(AppConfig::default(), tx, None);
        let result = engine.send_text_to_group("Test", "Hello").await;
        assert!(result.is_err());
        assert!(
            result.unwrap_err().to_string().contains("History"),
            "Expected error mentioning History"
        );
    }

    #[tokio::test]
    async fn test_send_text_to_group_with_history() {
        let path = format!(
            "/tmp/test_feix_eng_grp_{}.sqlite3",
            std::process::id()
        );
        let (tx, _rx) = mpsc::unbounded_channel();
        let engine = Engine::new(
            AppConfig::default(),
            tx,
            Some(std::path::PathBuf::from(&path)),
        );

        // Create a group
        engine
            .create_group("Team", &["10.0.0.1".into(), "10.0.0.2".into()])
            .unwrap();

        // send_text_to_group should work even without active network transport
        let result = engine.send_text_to_group("Team", "Hello world").await;
        assert!(
            result.is_ok(),
            "send_text_to_group failed: {:?}",
            result.err()
        );

        // Verify history was saved under group:Team key
        let msgs = engine.get_chat_history("group:Team", 0, 10).unwrap();
        assert!(
            !msgs.is_empty(),
            "No messages found under group:Team key"
        );
        assert_eq!(msgs.len(), 1);

        // Verify message content includes the group prefix
        assert!(
            msgs[0].content_json.contains("[Team] Hello world"),
            "Message content missing group prefix: {}",
            msgs[0].content_json
        );

        let _ = std::fs::remove_file(&path);
    }

    #[tokio::test]
    async fn test_send_text_to_group_nonexistent() {
        let path = format!(
            "/tmp/test_feix_eng_grp_nx_{}.sqlite3",
            std::process::id()
        );
        let (tx, _rx) = mpsc::unbounded_channel();
        let engine = Engine::new(
            AppConfig::default(),
            tx,
            Some(std::path::PathBuf::from(&path)),
        );

        let result = engine.send_text_to_group("Nonexistent", "Hello").await;
        assert!(result.is_err());
        assert!(
            result.unwrap_err().to_string().contains("not found"),
            "Expected 'not found' error"
        );

        let _ = std::fs::remove_file(&path);
    }

    // ─── File Task Management Tests ────────────────────────

    #[test]
    fn test_register_file_task() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let engine = Engine::new(AppConfig::default(), tx, None);

        let content = FileContent {
            file_id: 1,
            filename: "test.txt".into(),
            path: "/tmp/test.txt".into(),
            size: 1024,
            modify_time: 1000,
            file_type: IPMSG_FILE_REGULAR,
            packet_no: 0,
            local_task_id: None,
        };
        let task = FileTaskHandle::new(42, "10.0.0.1".into(), "Alice".into(), content, FileTaskType::Upload);

        let task_id = engine.register_file_task(task);
        assert_eq!(task_id, 42);

        let retrieved = engine.get_file_task(42);
        assert!(retrieved.is_some());
        let snap = retrieved.unwrap().snapshot();
        assert_eq!(snap.id, 42);
        assert_eq!(snap.fellow_ip, "10.0.0.1");
        assert_eq!(snap.task_type, FileTaskType::Upload);
    }

    #[test]
    fn test_get_file_task_not_found() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let engine = Engine::new(AppConfig::default(), tx, None);

        let retrieved = engine.get_file_task(999);
        assert!(retrieved.is_none());
    }

    #[test]
    fn test_cancel_file_task() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let engine = Engine::new(AppConfig::default(), tx, None);

        let content = FileContent {
            file_id: 1,
            filename: "test.txt".into(),
            path: "/tmp/test.txt".into(),
            size: 1024,
            modify_time: 1000,
            file_type: IPMSG_FILE_REGULAR,
            packet_no: 0,
            local_task_id: None,
        };
        let task = FileTaskHandle::new(7, "10.0.0.2".into(), "Bob".into(), content, FileTaskType::Download);
        engine.register_file_task(task);

        engine.cancel_file_task(7);

        let retrieved = engine.get_file_task(7).unwrap();
        let snap = retrieved.snapshot();
        assert_eq!(snap.state, FileTaskState::Canceled);
    }

    #[test]
    fn test_search_chat_history_no_db() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let engine = Engine::new(AppConfig::default(), tx, None);

        let results = engine.search_chat_history("hello", 10).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_search_chat_history_with_db() {
        use crate::storage::history::HistoryDb;
        let path = format!(
            "/tmp/test_feix_eng_search_{}.sqlite3",
            std::process::id()
        );

        // Create history DB and insert some data
        {
            let db = HistoryDb::open(std::path::Path::new(&path)).unwrap();
            let contents = vec![Content::Text {
                text: "Hello world!".into(),
                format: String::new(),
            }];
            db.save_message("10.0.0.1", "Alice", 0, &contents).unwrap();
        }

        let (tx, _rx) = mpsc::unbounded_channel();
        let engine = Engine::new(
            AppConfig::default(),
            tx,
            Some(std::path::PathBuf::from(&path)),
        );

        let results = engine.search_chat_history("Hello", 10).unwrap();
        assert!(!results.is_empty(), "Should find messages with 'Hello'");
        assert!(results[0].content_json.contains("Hello"));

        let no_results = engine.search_chat_history("NonExistent", 10).unwrap();
        assert!(no_results.is_empty());

        let _ = std::fs::remove_file(&path);
    }

    // ─── Engine Lifecycle Tests ──────────────────────────────

    #[tokio::test]
    async fn test_engine_restart() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let mut config = AppConfig::default();
        config.mode = ConnectionMode::RelayOnly;
        config.relay_server_url = String::new();
        let mut engine = Engine::new(config, tx, None);
        let result = engine.start().await;
        assert!(result.is_ok(), "First start failed: {:?}", result.err());
        engine.stop().await;
        let result = engine.start().await;
        assert!(result.is_ok(), "Restart failed: {:?}", result.err());
        engine.stop().await;
    }

    #[tokio::test]
    async fn test_engine_stop_is_idempotent() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let mut config = AppConfig::default();
        config.mode = ConnectionMode::RelayOnly;
        config.relay_server_url = String::new();
        let mut engine = Engine::new(config, tx, None);
        engine.stop().await;
        engine.start().await.unwrap();
        engine.stop().await;
        engine.start().await.unwrap();
        engine.stop().await;
        engine.stop().await;
        engine.start().await.unwrap();
        engine.stop().await;
    }
}


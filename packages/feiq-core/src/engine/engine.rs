//! FeiqEngine — the main Controller (MVC pattern).
//! Coordinates network, model, and storage. Dispatches events to frontend.
//! Mirrors feiqengine.cpp.

use crate::engine::events::FrontendEvent;
use crate::engine::tasks::FileTaskHandle;
use crate::model::contacts::ContactBook;
use crate::network::manager::{NetworkEvent, NetworkManager};
use crate::protocol::constants::*;
use crate::protocol::encoding::*;
use crate::protocol::serializer::*;
use crate::protocol::types::*;
use crate::storage::settings::AppConfig;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

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
    contacts: ContactBook,
    network: Option<NetworkManager>,
    event_tx: mpsc::UnboundedSender<FrontendEvent>,
    packet_id: PacketIdGen,
    file_id: FileIdGen,
    version: String,
    #[allow(dead_code)]
    file_tasks: HashMap<u64, Arc<FileTaskHandle>>,
}

impl Engine {
    /// Create a new engine (does not start networking yet)
    pub fn new(
        config: AppConfig,
        event_tx: mpsc::UnboundedSender<FrontendEvent>,
    ) -> Self {
        // Build version string: "feiq_plus_plus#128#MAC#0#0#0#1#9"
        let mac = mac_address::get_mac_address()
            .ok()
            .flatten()
            .map(|ma| ma.to_string().replace(':', ""))
            .unwrap_or_default();
        let version = format!("feiq_plus_plus#128#{mac}#0#0#0#1#9");

        Self {
            config,
            contacts: ContactBook::new(),
            network: None,
            event_tx,
            packet_id: PacketIdGen::new(),
            file_id: FileIdGen::new(),
            version,
            file_tasks: HashMap::new(),
        }
    }

    /// Get the version string used in protocol headers
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Start the engine: bind network, broadcast online presence
    pub async fn start(&mut self) -> anyhow::Result<()> {
        if self.network.is_some() {
            anyhow::bail!("Engine already started");
        }

        let (net_tx, mut net_rx) = mpsc::unbounded_channel::<NetworkEvent>();

        let mut network = NetworkManager::new(net_tx, self.config.name.clone(), self.config.port).await?;
        let self_mac = network.self_mac().to_string();

        // Update version with actual MAC
        self.version = format!("feiq_plus_plus#128#{self_mac}#0#0#0#1#9");

        // Send online broadcast on own port
        let online_data = build_br_entry(&self.config.name, &self.config.host, &self.version);
        network.broadcast(&online_data).await?;

        // If on non-standard port, also broadcast to default port 2425
        // so standard-port peers can discover us
        if self.config.port != 2425 {
            network.broadcast_to_port(2425, &online_data).await?;
        }

        // Broadcast to custom IP ranges (always on their own port, guessing 2425)
        for ip in &self.config.custom_ips {
            let _ = network.send_to(ip, 2425, &online_data).await;
        }

        self.network = Some(network);

        // Event dispatch loop
        let event_tx = self.event_tx.clone();
        let contacts = self.contacts.clone_arc();
        let config = self.config.clone();

        tokio::spawn(async move {
            while let Some(event) = net_rx.recv().await {
                handle_network_event(event, &event_tx, &contacts, &config);
            }
        });

        tracing::info!("Engine started: name={}, version={}", self.config.name, self.version);
        Ok(())
    }

    /// Get all known contacts (for frontend)
    pub fn get_contacts(&self) -> Vec<Fellow> {
        self.contacts.all()
    }

    /// Search contacts by query
    pub fn search_contacts(&self, query: &str) -> Vec<Fellow> {
        self.contacts.search(query)
    }

    /// Get a contact by IP
    pub fn find_contact(&self, ip: &str) -> Option<Fellow> {
        self.contacts.find_by_ip(ip)
    }

    /// Add a contact manually (user-added by IP)
    pub fn add_contact(&mut self, ip: &str) -> Fellow {
        let fellow = Fellow::new(ip);
        self.contacts.upsert(fellow.clone());
        fellow
    }

    /// Add a contact with custom port
    pub fn add_contact_with_port(&mut self, ip: &str, port: u16) -> Fellow {
        let mut fellow = Fellow::new(ip);
        fellow.port = port;
        self.contacts.upsert(fellow.clone());
        fellow
    }

    /// Send text message over the network
    pub async fn send_text_to(&self, ip: &str, port: u16, text: &str) -> anyhow::Result<()> {
        if let Some(ref network) = self.network {
            let data = build_text_message(
                self.packet_id(),
                &self.config.name,
                &self.config.host,
                &self.version,
                text,
            );
            network.send_to(ip, port, &data).await?;
        }
        Ok(())
    }

    /// Send knock over the network
    pub async fn send_knock_to(&self, ip: &str, port: u16) -> anyhow::Result<()> {
        if let Some(ref network) = self.network {
            let data = build_knock(&self.config.name, &self.config.host, &self.version);
            network.send_to(ip, port, &data).await?;
        }
        Ok(())
    }

    /// Generate next packet ID
    fn packet_id(&self) -> u64 {
        // Simple: use timestamp millis as ID
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }
}

// ─── Network event handler (runs in tokio task) ──────────────

fn handle_network_event(
    event: NetworkEvent,
    event_tx: &mpsc::UnboundedSender<FrontendEvent>,
    contacts: &Arc<Mutex<ContactBook>>,
    _config: &AppConfig,
) {
    match event {
        NetworkEvent::FellowOnline(post) | NetworkEvent::FellowAnsEntry(post) => {
            let fellow = post.from;
            let mut book = contacts.lock().unwrap();
            let is_new = book.find_by_ip(&fellow.ip).is_none();
            let changed = book.upsert(fellow.clone());

            let mut display_fellow = book.find_by_ip(&fellow.ip)
                .unwrap_or(fellow);
            display_fellow.online = true;

            if is_new || changed {
                let _ = event_tx.send(FrontendEvent::ContactUpdate {
                    fellow: display_fellow,
                });
            }
        }
        NetworkEvent::FellowOffline(post) => {
            let mut fellow = post.from;
            fellow.online = false;
            let mut book = contacts.lock().unwrap();
            book.upsert(fellow.clone());
            let _ = event_tx.send(FrontendEvent::ContactUpdate { fellow });
        }
        NetworkEvent::Message(mut post) => {
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
                    _ => true,
                }
            });

            if !post.contents.is_empty() {
                let timestamp = post.when.timestamp_millis();
                let _ = event_tx.send(FrontendEvent::NewMessage {
                    from_ip: post.from.ip.clone(),
                    from_name: post.from.display_name().to_string(),
                    contents: std::mem::take(&mut post.contents),
                    timestamp,
                });

                // Update contact book
                let mut book = contacts.lock().unwrap();
                book.upsert(post.from);
            }
        }
        NetworkEvent::Error(msg) => {
            let _ = event_tx.send(FrontendEvent::Error(msg));
        }
    }
}

// ─── Protocol message builders ───────────────────────────────

/// Build IPMSG_BR_ENTRY broadcast message
pub fn build_br_entry(name: &str, host: &str, version: &str) -> Vec<u8> {
    let name_gbk = encode_gbk(name);
    pack_message(
        0, // packet_no = 0 for broadcast
        name,
        host,
        version,
        IPMSG_BR_ENTRY,
        &name_gbk,
    )
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

/// Build IPMSG_ANSENTRY reply
pub fn build_ans_entry(name: &str, host: &str, version: &str) -> Vec<u8> {
    let name_gbk = encode_gbk(name);
    pack_message(
        0,
        name,
        host,
        version,
        IPMSG_ANSENTRY,
        &name_gbk,
    )
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
    })
}

use std::io::Write as IoWrite;

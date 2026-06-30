//! Core data types for feiq++ protocol
//! Mirrors the original feiq Fellow/Content/Post/FileContent types

use serde::{Deserialize, Serialize};

/// Where a peer was discovered from — determines which transport to use
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PeerSource {
    /// Discovered via LAN UDP broadcast (default)
    LanPeer,
    /// Discovered via relay server; String is the relay-assigned client_id
    RelayPeer(String),
}

impl Default for PeerSource {
    fn default() -> Self {
        Self::LanPeer
    }
}

/// A LAN user (friend/contact)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fellow {
    pub ip: String,
    #[serde(default)]
    pub pc_name: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub host: String,
    #[serde(default)]
    pub mac: String,
    #[serde(default)]
    pub online: bool,
    #[serde(default)]
    pub version: String,
    /// Custom display name set by user
    #[serde(default)]
    pub alias: String,
    /// Group this contact belongs to
    #[serde(default)]
    pub group_name: String,
    /// Personal signature
    #[serde(default)]
    pub signature: String,
    /// UDP port (default 2425, for multi-instance testing)
    #[serde(default = "default_fellow_port")]
    pub port: u16,
    /// Transport source — LAN or Relay
    #[serde(default)]
    pub source: PeerSource,
    /// Peer's x25519 public key (32 bytes) for ECDH key exchange (feiq++ only)
    #[serde(default)]
    pub public_key: Vec<u8>,
}

fn default_fellow_port() -> u16 { 2425 }

impl Fellow {
    /// Create a new fellow with just IP (rest discovered later)
    pub fn new(ip: impl Into<String>) -> Self {
        Self {
            ip: ip.into(),
            pc_name: String::new(),
            name: String::new(),
            host: String::new(),
            mac: String::new(),
            online: false,
            version: String::new(),
            alias: String::new(),
            group_name: String::new(),
            signature: String::new(),
            port: 2425,
            source: PeerSource::default(),
            public_key: Vec::new(),
        }
    }

    /// Display name: alias > name > pc_name
    pub fn display_name(&self) -> &str {
        if !self.alias.is_empty() {
            &self.alias
        } else if !self.name.is_empty() {
            &self.name
        } else if !self.pc_name.is_empty() {
            &self.pc_name
        } else {
            &self.ip
        }
    }

    /// Update fields from another fellow (for merge)
    pub fn update(&mut self, other: &Fellow) -> bool {
        let mut changed = false;

        if !other.name.is_empty() && self.name != other.name {
            self.name = other.name.clone();
            changed = true;
        }

        if !other.mac.is_empty() && self.mac != other.mac {
            self.mac = other.mac.clone();
            changed = true;
        }

        if self.online != other.online {
            self.online = other.online;
            changed = true;
        }

        changed
    }

    /// Two fellows are the same if they share IP or MAC
    pub fn is_same(&self, other: &Fellow) -> bool {
        self.ip == other.ip || (!self.mac.is_empty() && self.mac == other.mac)
    }
}

/// File content in a message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileContent {
    pub file_id: u64,
    pub filename: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub size: i64,
    #[serde(default)]
    pub modify_time: i64,
    #[serde(default)]
    pub file_type: u32,
    /// Packet number this file belongs to
    #[serde(default)]
    pub packet_no: u64,
    /// Local task ID assigned by the engine for tracking this file transfer
    #[serde(default)]
    pub local_task_id: Option<u64>,
}

/// Parsed GETFILEDATA request from a remote peer
#[derive(Debug, Clone)]
pub struct GetFileData {
    pub packet_no: u64,
    pub file_id: u64,
    pub offset: i64,
}

/// Message content enum
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Content {
    #[serde(rename = "text")]
    Text {
        text: String,
        #[serde(default)]
        format: String,
    },
    #[serde(rename = "knock")]
    Knock,
    #[serde(rename = "file")]
    File(FileContent),
    #[serde(rename = "image")]
    Image {
        /// 8-char image ID (legacy protocol)
        id: String,
    },
    #[serde(rename = "id")]
    Id {
        /// Reference packet ID (for read receipts)
        id: u64,
    },
    #[serde(rename = "sealed")]
    Sealed {
        text: String,
        #[serde(default)]
        format: String,
        #[serde(default)]
        ttl_seconds: u32,
    },
}

impl Content {
    pub fn content_type(&self) -> &str {
        match self {
            Content::Text { .. } => "text",
            Content::Knock => "knock",
            Content::File(_) => "file",
            Content::Image { .. } => "image",
            Content::Id { .. } => "id",
            Content::Sealed { .. } => "sealed",
        }
    }

    pub fn is_text(&self) -> bool {
        matches!(self, Content::Text { .. })
    }

    pub fn is_file(&self) -> bool {
        matches!(self, Content::File(_))
    }

    pub fn is_knock(&self) -> bool {
        matches!(self, Content::Knock)
    }
}

/// A parsed IPMSG post (one network datagram)
#[derive(Debug, Clone)]
pub struct Post {
    /// Reception time
    pub when: chrono::DateTime<chrono::Utc>,
    /// Raw extra data after the header
    pub extra: Vec<u8>,
    /// Packet sequence number
    pub packet_no: String,
    /// Command ID (cmd | options)
    pub cmd_id: u32,
    /// Sender info
    pub from: Fellow,
    /// Parsed contents
    pub contents: Vec<Content>,
    /// Parsed GETFILEDATA request (if cmd_id is IPMSG_GETFILEDATA)
    pub get_file_data: Option<GetFileData>,
}

impl Post {
    pub fn new(ip: impl Into<String>) -> Self {
        Self {
            when: chrono::Utc::now(),
            extra: Vec::new(),
            packet_no: String::new(),
            cmd_id: 0,
            from: Fellow::new(ip),
            contents: Vec::new(),
            get_file_data: None,
        }
    }

    pub fn add_content(&mut self, content: Content) {
        self.contents.push(content);
    }
}

/// Version info extracted from the version string
#[derive(Debug, Clone, Default)]
pub struct VersionInfo {
    pub mac: String,
    pub version: String,
}

/// File transfer direction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileTaskType {
    #[serde(rename = "upload")]
    Upload,
    #[serde(rename = "download")]
    Download,
}

/// File transfer state
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileTaskState {
    #[serde(rename = "not_start")]
    NotStart,
    #[serde(rename = "running")]
    Running,
    #[serde(rename = "finish")]
    Finish,
    #[serde(rename = "error")]
    Error(String),
    #[serde(rename = "canceled")]
    Canceled,
}

/// A file transfer task
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTask {
    pub id: u64,
    pub fellow_ip: String,
    pub fellow_name: String,
    pub content: FileContent,
    pub task_type: FileTaskType,
    pub state: FileTaskState,
    pub progress: i64,
    pub total: i64,
    #[serde(default)]
    pub cancel_pending: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_content_local_task_id_default() {
        // local_task_id should default to None when deserializing
        let json = r#"{"file_id": 42, "filename": "test.txt", "size": 1024}"#;
        let fc: FileContent = serde_json::from_str(json).unwrap();
        assert_eq!(fc.file_id, 42);
        assert_eq!(fc.filename, "test.txt");
        assert_eq!(fc.size, 1024);
        assert!(fc.local_task_id.is_none());
    }

    #[test]
    fn test_file_content_local_task_id_set() {
        // local_task_id should be deserialized when present
        let json = r#"{"file_id": 7, "filename": "photo.jpg", "local_task_id": 123}"#;
        let fc: FileContent = serde_json::from_str(json).unwrap();
        assert_eq!(fc.local_task_id, Some(123));
    }

    #[test]
    fn test_file_content_local_task_id_serialize_roundtrip() {
        // Roundtrip: serialize then deserialize preserves local_task_id
        let fc = FileContent {
            file_id: 1,
            filename: "doc.pdf".into(),
            path: String::new(),
            size: 5000,
            modify_time: 1000,
            file_type: 1,
            packet_no: 0,
            local_task_id: Some(42),
        };
        let json = serde_json::to_string(&fc).unwrap();
        let fc2: FileContent = serde_json::from_str(&json).unwrap();
        assert_eq!(fc2.local_task_id, Some(42));
    }

    #[test]
    fn test_get_file_data_struct() {
        let gfd = GetFileData {
            packet_no: 12345,
            file_id: 67890,
            offset: 0,
        };
        assert_eq!(gfd.packet_no, 12345);
        assert_eq!(gfd.file_id, 67890);
        assert_eq!(gfd.offset, 0);
    }

    #[test]
    fn test_post_get_file_data_default() {
        let post = Post::new("192.168.1.1");
        assert!(post.get_file_data.is_none());
    }

    #[test]
    fn test_content_sealed_roundtrip() {
        let sealed = Content::Sealed {
            text: "burn after reading".into(),
            format: String::new(),
            ttl_seconds: 60,
        };
        let json = serde_json::to_string(&sealed).unwrap();
        let deserialized: Content = serde_json::from_str(&json).unwrap();
        match deserialized {
            Content::Sealed { text, ttl_seconds, .. } => {
                assert_eq!(text, "burn after reading");
                assert_eq!(ttl_seconds, 60);
            }
            _ => panic!("Expected Sealed content"),
        }
    }

    #[test]
    fn test_content_sealed_default_ttl() {
        let json = r#"{"type": "sealed", "text": "self-destruct"}"#;
        let deserialized: Content = serde_json::from_str(json).unwrap();
        match deserialized {
            Content::Sealed { text, ttl_seconds, .. } => {
                assert_eq!(text, "self-destruct");
                assert_eq!(ttl_seconds, 0);
            }
            _ => panic!("Expected Sealed content"),
        }
    }

    #[test]
    fn test_fellow_public_key_default() {
        let json = r#"{"ip": "10.0.0.1", "name": "Alice"}"#;
        let fellow: Fellow = serde_json::from_str(json).unwrap();
        assert!(fellow.public_key.is_empty());
    }

    #[test]
    fn test_fellow_public_key_deserialize() {
        let json = r#"{"ip": "10.0.0.1", "name": "Alice", "public_key": [1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32]}"#;
        let fellow: Fellow = serde_json::from_str(json).unwrap();
        assert_eq!(fellow.public_key.len(), 32);
        assert_eq!(fellow.public_key[0], 1);
        assert_eq!(fellow.public_key[31], 32);
    }
}

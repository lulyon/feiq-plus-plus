//! Core data types for feiq++ protocol
//! Mirrors the original feiq Fellow/Content/Post/FileContent types

use serde::{Deserialize, Serialize};

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
}

impl Content {
    pub fn content_type(&self) -> &str {
        match self {
            Content::Text { .. } => "text",
            Content::Knock => "knock",
            Content::File(_) => "file",
            Content::Image { .. } => "image",
            Content::Id { .. } => "id",
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

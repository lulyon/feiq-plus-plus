//! Frontend events emitted by the engine.
//! These are serialized and sent to the React frontend via Tauri events.

use crate::protocol::types::*;
use serde::Serialize;

/// Events pushed from engine to frontend (Tauri event system)
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum FrontendEvent {
    /// A contact was updated (online/offline/name change)
    #[serde(rename = "contact_update")]
    ContactUpdate { fellow: Fellow },
    /// New message(s) received from a contact
    #[serde(rename = "new_message")]
    NewMessage {
        from_ip: String,
        from_name: String,
        contents: Vec<Content>,
        /// Timestamp in milliseconds since epoch
        timestamp: i64,
    },
    /// A sent message timed out (no RECVMSG confirmation)
    #[serde(rename = "send_timeout")]
    SendTimeout {
        to_ip: String,
        content: Content,
    },
    /// File transfer progress update
    #[serde(rename = "file_progress")]
    FileProgress {
        task_id: u64,
        progress: i64,
        total: i64,
    },
    /// File transfer state changed
    #[serde(rename = "file_state_changed")]
    FileStateChanged {
        task_id: u64,
        state: FileTaskState,
        message: String,
    },
    /// Engine error
    #[serde(rename = "engine_error")]
    Error(String),
    /// A sealed message we sent was read by the recipient
    #[serde(rename = "message_read_confirmed")]
    MessageReadConfirmed {
        from_ip: String,
        from_name: String,
        packet_no: u64,
    },
    /// A peer is typing or stopped typing
    #[serde(rename = "typing_indicator")]
    TypingIndicator {
        from_ip: String,
        from_name: String,
        is_typing: bool,
    },
}

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
    /// Folder transfer progress update
    #[serde(rename = "folder_progress")]
    FolderProgress {
        task_id: u64,
        /// Total bytes transferred so far (across all files)
        overall_progress: i64,
        /// Total bytes for all files
        overall_total: i64,
        /// Current file's relative path within the folder
        current_file: String,
        /// Current file progress in bytes
        current_file_progress: i64,
        /// Current file total size in bytes
        current_file_total: i64,
        /// Number of files completed (including current if finished)
        files_completed: u32,
        /// Total number of files in the folder
        total_files: u32,
    },
    /// Folder transfer state changed
    #[serde(rename = "folder_state_changed")]
    FolderStateChanged {
        task_id: u64,
        state: FileTaskState,
        message: String,
    },
    /// Engine error
    #[serde(rename = "engine_error")]
    Error(String),
}

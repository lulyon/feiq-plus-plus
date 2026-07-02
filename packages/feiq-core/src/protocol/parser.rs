//! Chain-of-responsibility protocol parser.
//! Each handler tries to parse the Post; returns true if chain should stop.
//! Mirrors feiqengine.cpp protocol chain.

use crate::protocol::constants::*;
use crate::protocol::encoding::*;
use crate::protocol::serializer::*;
use crate::protocol::types::*;

/// A protocol handler in the chain.
/// Returns true if the chain should stop (no further handlers called).
pub trait RecvProtocol: Send + Sync {
    fn name(&self) -> &str;
    fn read(&self, post: &mut Post, chain: &ProtocolChain) -> bool;
}

/// Manages the chain of protocol handlers
pub struct ProtocolChain {
    handlers: Vec<Box<dyn RecvProtocol>>,
}

impl ProtocolChain {
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
        }
    }

    pub fn add_handler(&mut self, handler: Box<dyn RecvProtocol>) {
        self.handlers.push(handler);
    }

    /// Run all handlers in sequence. Each handler gets a mutable reference to the Post.
    /// If any handler returns true, the chain stops.
    pub fn process(&self, post: &mut Post) {
        for handler in &self.handlers {
            if handler.read(post, self) {
                break;
            }
        }
    }
}

impl Default for ProtocolChain {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Debunger: prints raw packet info (development only) ─────

pub struct DebugHandler;

impl RecvProtocol for DebugHandler {
    fn name(&self) -> &str {
        "Debug"
    }

    fn read(&self, post: &mut Post, _chain: &ProtocolChain) -> bool {
        if tracing::enabled!(tracing::Level::TRACE) {
            tracing::trace!(
                "Packet from {}: cmd=0x{:X}, extra_len={}, contents={}",
                post.from.ip,
                post.cmd_id,
                post.extra.len(),
                post.contents.len(),
            );
        }
        false // never stop chain
    }
}

/// Extract feiq++ public key from extra data and strip it.
/// Format: [GBK name bytes] [NUL] [32-byte pub key]
fn extract_peer_public_key(post: &mut Post) {
    if !post.from.version.starts_with("feiq_plus_plus") {
        return;
    }
    if post.extra.len() < 33 {
        return;
    }
    if let Some(nul_pos) = post.extra.iter().position(|&b| b == 0) {
        if post.extra.len() >= nul_pos + 1 + 32 {
            let key_start = nul_pos + 1;
            post.from.public_key = post.extra[key_start..key_start + 32].to_vec();
            post.extra.truncate(nul_pos); // Remove NUL and pub key, keep name only
        }
    }
}

// ─── RecvAnsEntry: handles IPMSG_ANSENTRY ────────────────────

pub struct RecvAnsEntry;

impl RecvProtocol for RecvAnsEntry {
    fn name(&self) -> &str {
        "RecvAnsEntry"
    }

    fn read(&self, post: &mut Post, _chain: &ProtocolChain) -> bool {
        if is_cmd_set(post.cmd_id, IPMSG_ANSENTRY) {
            // Extract public key if feiq++ peer
            extract_peer_public_key(post);

            let is_utf8 = is_opt_set(post.cmd_id, IPMSG_UTF8OPT);
            let name = decode_by_utf8opt(&post.extra, is_utf8);
            if !name.is_empty() {
                post.from.name = name;
            }
            return true; // fully handled
        }
        false
    }
}

// ─── RecvBrEntry: handles IPMSG_BR_ENTRY (user online) ──────

pub struct RecvBrEntry;

impl RecvProtocol for RecvBrEntry {
    fn name(&self) -> &str {
        "RecvBrEntry"
    }

    fn read(&self, post: &mut Post, _chain: &ProtocolChain) -> bool {
        if is_cmd_set(post.cmd_id, IPMSG_BR_ENTRY) {
            // Extract public key if feiq++ peer
            extract_peer_public_key(post);

            let is_utf8 = is_opt_set(post.cmd_id, IPMSG_UTF8OPT);
            let name = decode_by_utf8opt(&post.extra, is_utf8);
            if !name.is_empty() {
                post.from.name = name;
            }
            return true; // fully handled
        }
        false
    }
}

// ─── RecvBrAbsence: handles IPMSG_BR_ABSENCE (name/status change) ──

pub struct RecvBrAbsence;

impl RecvProtocol for RecvBrAbsence {
    fn name(&self) -> &str {
        "RecvBrAbsence"
    }

    fn read(&self, post: &mut Post, _chain: &ProtocolChain) -> bool {
        if is_cmd_set(post.cmd_id, IPMSG_BR_ABSENCE) {
            // Extract public key if feiq++ peer
            extract_peer_public_key(post);

            let is_utf8 = is_opt_set(post.cmd_id, IPMSG_UTF8OPT);
            let name = decode_by_utf8opt(&post.extra, is_utf8);
            if !name.is_empty() {
                post.from.name = name;
            }
            return true; // fully handled
        }
        false
    }
}

// ─── RecvBrExit: handles IPMSG_BR_EXIT (user offline) ───────

pub struct RecvBrExit;

impl RecvProtocol for RecvBrExit {
    fn name(&self) -> &str {
        "RecvBrExit"
    }

    fn read(&self, post: &mut Post, _chain: &ProtocolChain) -> bool {
        if is_cmd_set(post.cmd_id, IPMSG_BR_EXIT) {
            post.from.online = false;
            return true;
        }
        false
    }
}

// ─── RecvInputing: handles IPMSG_INPUTING (typing indicator) ──

pub struct RecvInputing;

impl RecvProtocol for RecvInputing {
    fn name(&self) -> &str {
        "RecvInputing"
    }

    fn read(&self, post: &mut Post, _chain: &ProtocolChain) -> bool {
        if is_cmd_set(post.cmd_id, IPMSG_INPUTING) {
            post.contents.push(Content::Typing { is_typing: true });
            return true;
        }
        false
    }
}

// ─── RecvInputEnd: handles IPMSG_INPUT_END (typing ended) ──

pub struct RecvInputEnd;

impl RecvProtocol for RecvInputEnd {
    fn name(&self) -> &str {
        "RecvInputEnd"
    }

    fn read(&self, post: &mut Post, _chain: &ProtocolChain) -> bool {
        if is_cmd_set(post.cmd_id, IPMSG_INPUT_END) {
            post.contents.push(Content::Typing { is_typing: false });
            return true;
        }
        false
    }
}

// ─── RecvKnock: handles IPMSG_KNOCK (window shake) ──────────

pub struct RecvKnock;

impl RecvProtocol for RecvKnock {
    fn name(&self) -> &str {
        "RecvKnock"
    }

    fn read(&self, post: &mut Post, _chain: &ProtocolChain) -> bool {
        if is_cmd_set(post.cmd_id, IPMSG_KNOCK) {
            post.contents.push(Content::Knock);
        }
        false // continue chain (EndRecv will trigger)
    }
}

// ─── RecvSendCheck: handles SENDCHECKOPT ─────────────────────

pub struct RecvSendCheck;

impl RecvProtocol for RecvSendCheck {
    fn name(&self) -> &str {
        "RecvSendCheck"
    }

    fn read(&self, _post: &mut Post, _chain: &ProtocolChain) -> bool {
        // Handled externally by checking is_opt_set(post.cmd_id, IPMSG_SENDCHECKOPT)
        // This is a marker handler - the actual reply logic is in the engine
        false
    }
}

// ─── RecvReadCheck: handles READCHECKOPT ─────────────────────

pub struct RecvReadCheck;

impl RecvProtocol for RecvReadCheck {
    fn name(&self) -> &str {
        "RecvReadCheck"
    }

    fn read(&self, post: &mut Post, _chain: &ProtocolChain) -> bool {
        if is_cmd_set(post.cmd_id, IPMSG_SENDMSG) && is_opt_set(post.cmd_id, IPMSG_SECRETEXOPT) {
            // Sealed message (read-and-destroy): parse text as sealed content
            let null_pos = post.extra.iter().position(|&b| b == 0);
            let text_bytes = match null_pos {
                Some(0) => return false, // starts with null = no text
                Some(pos) => &post.extra[..pos],
                None => &post.extra[..],
            };
            if !text_bytes.is_empty() {
                let is_utf8 = is_opt_set(post.cmd_id, IPMSG_UTF8OPT);
                let raw_text = decode_by_utf8opt(text_bytes, is_utf8);
                post.contents.push(Content::Sealed {
                    text: raw_text,
                    format: String::new(),
                    ttl_seconds: 60,
                });
            }
            return true; // sealed content handled, stop chain
        }
        false
    }
}

// ─── RecvText: handles IPMSG_SENDMSG text body ───────────────

pub struct RecvText;

impl RecvProtocol for RecvText {
    fn name(&self) -> &str {
        "RecvText"
    }

    fn read(&self, post: &mut Post, _chain: &ProtocolChain) -> bool {
        if !is_cmd_set(post.cmd_id, IPMSG_SENDMSG) {
            return false;
        }

        // Text data starts after the first null byte (file data follows)
        // If no null byte, entire extra is text
        let null_pos = post.extra.iter().position(|&b| b == 0);

        let text_bytes = match null_pos {
            Some(0) => return false, // starts with null = no text
            Some(pos) => &post.extra[..pos],
            None => &post.extra[..],
        };

        if !text_bytes.is_empty() {
            let is_utf8 = is_opt_set(post.cmd_id, IPMSG_UTF8OPT);
            let raw_text = decode_by_utf8opt(text_bytes, is_utf8);
            let content = parse_text_content(&raw_text);
            post.contents.push(content);
        }

        false // continue to let RecvFile also parse
    }
}

/// Parse text message with optional {format} suffix.
/// The format suffix, if present, is at the END of the message (FeiQ convention).
fn parse_text_content(raw: &str) -> Content {
    // Check for {format} suffix at the END: text{format}
    // Use rfind to match the LAST '{' to avoid treating user-entered braces
    // in the middle of the message as format delimiters.
    if let Some(begin) = raw.rfind('{') {
        if let Some(end) = raw[begin + 1..].find('}') {
            let text = raw[..begin].to_string();
            let format = raw[begin + 1..begin + 1 + end].to_string();
            return Content::Text { text, format };
        }
    }

    Content::Text {
        text: raw.to_string(),
        format: String::new(),
    }
}

// ─── RecvFile: handles IPMSG_FILEATTACHOPT file data ─────────

pub struct RecvFile;

impl RecvProtocol for RecvFile {
    fn name(&self) -> &str {
        "RecvFile"
    }

    fn read(&self, post: &mut Post, _chain: &ProtocolChain) -> bool {
        if !is_opt_set(post.cmd_id, IPMSG_FILEATTACHOPT) || !is_cmd_set(post.cmd_id, IPMSG_SENDMSG)
        {
            return false;
        }

        // File data follows the text portion after the first null byte
        let null_pos = match post.extra.iter().position(|&b| b == 0) {
            Some(pos) => pos + 1,
            None => 0,
        };

        if null_pos == 0 || null_pos >= post.extra.len() {
            tracing::debug!(
                "RecvFile: malformed FILEATTACHOPT data: null_pos={}, extra_len={}, from={}",
                null_pos,
                post.extra.len(),
                post.from.ip,
            );
            return false;
        }

        let file_data = &post.extra[null_pos..];

        // Each file task is separated by FILELIST_SEPARATOR (0x07)
        // Format: fileId:filename:size:modifyTime:fileType:...\x07
        let mut start = 0;
        while start < file_data.len() {
            let end = file_data[start..]
                .iter()
                .position(|&b| b == FILELIST_SEPARATOR)
                .map(|p| start + p)
                .unwrap_or(file_data.len());

            if end <= start {
                break;
            }

            let task_bytes = &file_data[start..end];
            let is_utf8 = is_opt_set(post.cmd_id, IPMSG_UTF8OPT);
            if let Some(content) = parse_file_task(task_bytes, is_utf8) {
                post.contents.push(Content::File(content));
            } else {
                tracing::debug!(
                    "RecvFile: parse_file_task returned None for entry: {:?}, from={}",
                    task_bytes,
                    post.from.ip,
                );
            }

            start = end + 1;
            if start >= file_data.len() {
                break;
            }
        }

        false
    }
}

/// Parse a single file task from raw bytes (may be GBK or UTF-8).
fn parse_file_task(data: &[u8], is_utf8: bool) -> Option<FileContent> {
    let values = split_allow_separator(data, HLIST_ENTRY_SEPARATOR);
    if values.len() < 5 {
        tracing::debug!(
            "parse_file_task: malformed entry: expected >=5 colon-separated fields, got {}: data={:?}",
            values.len(),
            data,
        );
        return None;
    }

    // Parse numeric fields: file_id, size, modify_time, file_type
    let file_id_str = String::from_utf8_lossy(&values[0]);
    let file_id: u64 = file_id_str.parse().ok()?;
    let size_str = String::from_utf8_lossy(&values[2]);
    let size: i64 = i64::from_str_radix(&size_str, 16).ok()?;
    let mtime_str = String::from_utf8_lossy(&values[3]);
    let modify_time: i64 = i64::from_str_radix(&mtime_str, 16).ok()?;
    let ftype_str = String::from_utf8_lossy(&values[4]);
    let file_type: u32 = u32::from_str_radix(&ftype_str, 16).ok()?;

    // Decode filename from raw bytes using the correct charset
    let filename = decode_by_utf8opt(&values[1], is_utf8);

    Some(FileContent {
        file_id,
        filename,
        path: String::new(),
        size,
        modify_time,
        file_type,
        packet_no: 0, // set by caller
        local_task_id: None,
    })
}

// ─── RecvImage: handles IPMSG_SENDIMAGE ──────────────────────

pub struct RecvImage;

impl RecvProtocol for RecvImage {
    fn name(&self) -> &str {
        "RecvImage"
    }

    fn read(&self, post: &mut Post, _chain: &ProtocolChain) -> bool {
        if is_cmd_set(post.cmd_id, IPMSG_SENDIMAGE)
            && is_opt_set(post.cmd_id, IPMSG_FILEATTACHOPT)
            && post.extra.len() >= 8
        {
            let id_bytes = &post.extra[..8];
            let id = String::from_utf8_lossy(id_bytes).into_owned();
            post.contents.push(Content::Image { id });
        }
        false
    }
}

// ─── RecvReadMessage: handles IPMSG_RECVMSG (read receipt) ───

pub struct RecvReadMessage;

impl RecvProtocol for RecvReadMessage {
    fn name(&self) -> &str {
        "RecvReadMessage"
    }

    fn read(&self, post: &mut Post, _chain: &ProtocolChain) -> bool {
        if is_cmd_set(post.cmd_id, IPMSG_RECVMSG) {
            let extra_str = String::from_utf8_lossy(&post.extra);
            if let Ok(id) = extra_str.trim().parse::<u64>() {
                post.contents.push(Content::Id { id });
            }
            return true;
        }
        false
    }
}

// ─── RecvReadMsgSealed: handles IPMSG_READMSG (sealed message read) ──

pub struct RecvReadMsgSealed;

impl RecvProtocol for RecvReadMsgSealed {
    fn name(&self) -> &str {
        "RecvReadMsgSealed"
    }

    fn read(&self, post: &mut Post, _chain: &ProtocolChain) -> bool {
        if is_cmd_set(post.cmd_id, IPMSG_READMSG) {
            let extra_str = String::from_utf8_lossy(&post.extra);
            if let Ok(id) = extra_str.trim().parse::<u64>() {
                post.contents.push(Content::Id { id });
            }
            return true;
        }
        false
    }
}

// ─── RecvAnsReadMsg: handles IPMSG_ANSREADMSG (read receipt ack) ──

pub struct RecvAnsReadMsg;

impl RecvProtocol for RecvAnsReadMsg {
    fn name(&self) -> &str {
        "RecvAnsReadMsg"
    }

    fn read(&self, post: &mut Post, _chain: &ProtocolChain) -> bool {
        if is_cmd_set(post.cmd_id, IPMSG_ANSREADMSG) {
            let extra_str = String::from_utf8_lossy(&post.extra);
            if let Ok(id) = extra_str.trim().parse::<u64>() {
                post.contents.push(Content::Id { id });
            }
            return true;
        }
        false
    }
}

// ─── RecvGetFileData: handles IPMSG_GETFILEDATA (file data request) ──

pub struct RecvGetFileData;

impl RecvProtocol for RecvGetFileData {
    fn name(&self) -> &str {
        "RecvGetFileData"
    }

    fn read(&self, post: &mut Post, _chain: &ProtocolChain) -> bool {
        if !is_cmd_set(post.cmd_id, IPMSG_GETFILEDATA) && !is_cmd_set(post.cmd_id, IPMSG_GETDIRFILES) {
            return false;
        }

        // Format: packetNo:fileId:offset:[:password]
        let extra_str = String::from_utf8_lossy(&post.extra);
        let parts: Vec<&str> = extra_str.trim_end_matches(':').split(':').collect();

        if parts.len() >= 3 {
            if let (Ok(packet_no), Ok(file_id), Ok(offset)) = (
                parts[0].parse::<u64>(),
                parts[1].parse::<u64>(),
                parts[2].parse::<i64>(),
            ) {
                let password = if is_opt_set(post.cmd_id, IPMSG_PASSWORDOPT) && parts.len() >= 4 {
                    Some(parts[3].to_string())
                } else {
                    None
                };
                post.get_file_data = Some(GetFileData {
                    packet_no,
                    file_id,
                    offset,
                    password,
                });
            }
        }

        true // stop chain
    }
}

// ─── RecvReleaseFiles: handles IPMSG_RELEASEFILES ───────────

pub struct RecvReleaseFiles;

impl RecvProtocol for RecvReleaseFiles {
    fn name(&self) -> &str {
        "RecvReleaseFiles"
    }

    fn read(&self, post: &mut Post, _chain: &ProtocolChain) -> bool {
        if !is_cmd_set(post.cmd_id, IPMSG_RELEASEFILES) {
            return false;
        }

        // Format: packetNo:fileId:\0
        let extra_str = String::from_utf8_lossy(&post.extra);
        let parts: Vec<&str> = extra_str.trim_end_matches(':').split(':').collect();
        if parts.len() >= 2 {
            if let (Ok(packet_no), Ok(file_id)) = (
                parts[0].parse::<u64>(),
                parts[1].parse::<u64>(),
            ) {
                post.get_file_data = Some(GetFileData {
                    packet_no,
                    file_id,
                    offset: 0,
                    password: None,
                });
            }
        }

        true // stop chain
    }
}

// ─── RecvGetAvatar: handles IPMSG_GETAVATAR (avatar request) ──

pub struct RecvGetAvatar;

impl RecvProtocol for RecvGetAvatar {
    fn name(&self) -> &str {
        "RecvGetAvatar"
    }

    fn read(&self, post: &mut Post, _chain: &ProtocolChain) -> bool {
        if is_cmd_set(post.cmd_id, IPMSG_GETAVATAR) {
            post.contents.push(Content::Text {
                text: "[AvatarRequest]".into(),
                format: String::new(),
            });
            return true;
        }
        false
    }
}

// ─── RecvSendAvatar: handles IPMSG_SENDAVATAR (avatar data) ──

pub struct RecvSendAvatar;

impl RecvProtocol for RecvSendAvatar {
    fn name(&self) -> &str {
        "RecvSendAvatar"
    }

    fn read(&self, post: &mut Post, _chain: &ProtocolChain) -> bool {
        if is_cmd_set(post.cmd_id, IPMSG_SENDAVATAR) {
            // Avatar data is in the extra field (SHA256:base64_image_data)
            post.contents.push(Content::Text {
                text: "[AvatarData]".into(),
                format: String::new(),
            });
            return true;
        }
        false
    }
}

// ─── EndRecv: terminal handler, triggers event if contents exist ──

pub struct EndRecv;

impl RecvProtocol for EndRecv {
    fn name(&self) -> &str {
        "EndRecv"
    }

    fn read(&self, _post: &mut Post, _chain: &ProtocolChain) -> bool {
        // Always stops the chain. The engine checks if post.contents is non-empty.
        true
    }
}

// ─── Builder ─────────────────────────────────────────────────

/// Build the standard protocol chain (matching original feiq)
pub fn build_default_chain() -> ProtocolChain {
    let mut chain = ProtocolChain::new();

    chain.add_handler(Box::new(DebugHandler));
    chain.add_handler(Box::new(RecvAnsEntry));
    chain.add_handler(Box::new(RecvBrEntry));
    chain.add_handler(Box::new(RecvBrAbsence));
    chain.add_handler(Box::new(RecvBrExit));
    chain.add_handler(Box::new(RecvSendCheck));
    chain.add_handler(Box::new(RecvReadCheck));
    chain.add_handler(Box::new(RecvReadMessage));
    chain.add_handler(Box::new(RecvReadMsgSealed));
    chain.add_handler(Box::new(RecvAnsReadMsg));
    chain.add_handler(Box::new(RecvText));
    chain.add_handler(Box::new(RecvImage));
    chain.add_handler(Box::new(RecvInputing));
    chain.add_handler(Box::new(RecvInputEnd));
    chain.add_handler(Box::new(RecvKnock));
    chain.add_handler(Box::new(RecvFile));
    chain.add_handler(Box::new(RecvGetFileData));
    chain.add_handler(Box::new(RecvReleaseFiles));
    chain.add_handler(Box::new(RecvGetAvatar));
    chain.add_handler(Box::new(RecvSendAvatar));
    chain.add_handler(Box::new(EndRecv));

    chain
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chain_br_absence() {
        let chain = build_default_chain();
        let mut post = Post::new("192.168.1.100");
        post.cmd_id = IPMSG_BR_ABSENCE;

        // "李四" in GBK
        let gbk_name = b"\xc0\xee\xcb\xc4".to_vec();
        post.extra = gbk_name;

        chain.process(&mut post);

        assert_eq!(post.from.name, "李四");
        assert!(post.contents.is_empty()); // BR_ABSENCE has no display content
    }

    #[test]
    fn test_chain_br_entry() {
        let chain = build_default_chain();
        let mut post = Post::new("192.168.1.100");
        post.cmd_id = IPMSG_BR_ENTRY;

        // "张三" in GBK
        let gbk_name = b"\xd5\xc5\xc8\xfd".to_vec();
        post.extra = gbk_name;

        chain.process(&mut post);

        assert_eq!(post.from.name, "张三");
        assert!(post.contents.is_empty()); // BR_ENTRY has no display content
    }

    #[test]
    fn test_chain_text_message() {
        let chain = build_default_chain();
        let mut post = Post::new("192.168.1.100");
        post.cmd_id = IPMSG_SENDMSG | IPMSG_SENDCHECKOPT;

        // "你好世界" in GBK
        let gbk_text = b"\xc4\xe3\xba\xc3\xca\xc0\xbd\xe7".to_vec();
        post.extra = gbk_text;

        chain.process(&mut post);

        assert_eq!(post.contents.len(), 1);
        match &post.contents[0] {
            Content::Text { text, .. } => assert_eq!(text, "你好世界"),
            _ => panic!("Expected Text content"),
        }
    }

    #[test]
    fn test_chain_get_file_data() {
        let chain = build_default_chain();
        let mut post = Post::new("192.168.1.100");
        post.cmd_id = IPMSG_GETFILEDATA;
        // Format: packetNo:fileId:offset:\0
        post.extra = b"12345:67890:0:\0".to_vec();

        chain.process(&mut post);

        assert!(post.get_file_data.is_some());
        let gfd = post.get_file_data.as_ref().unwrap();
        assert_eq!(gfd.packet_no, 12345);
        assert_eq!(gfd.file_id, 67890);
        assert_eq!(gfd.offset, 0);
    }

    #[test]
    fn test_chain_get_dir_files() {
        let chain = build_default_chain();
        let mut post = Post::new("10.0.0.1");
        post.cmd_id = IPMSG_GETDIRFILES;
        // Format: packetNo:fileId:offset:
        post.extra = b"100:0:1:\0".to_vec();

        chain.process(&mut post);

        assert!(post.get_file_data.is_some());
        let gfd = post.get_file_data.as_ref().unwrap();
        assert_eq!(gfd.packet_no, 100);
        assert_eq!(gfd.file_id, 0);
        assert_eq!(gfd.offset, 1);
    }

    #[test]
    fn test_chain_get_file_data_non_zero_offset() {
        let chain = build_default_chain();
        let mut post = Post::new("10.0.0.1");
        post.cmd_id = IPMSG_GETFILEDATA;
        // Format: packetNo:fileId:offset:
        post.extra = b"999:1:65536:\0".to_vec();

        chain.process(&mut post);

        assert!(post.get_file_data.is_some());
        let gfd = post.get_file_data.as_ref().unwrap();
        assert_eq!(gfd.packet_no, 999);
        assert_eq!(gfd.file_id, 1);
        assert_eq!(gfd.offset, 65536);
    }

    #[test]
    fn test_chain_get_file_data_not_matched() {
        // Regular SENDMSG should NOT set get_file_data
        let chain = build_default_chain();
        let mut post = Post::new("192.168.1.100");
        post.cmd_id = IPMSG_SENDMSG;
        let gbk_text = b"\xc4\xe3\xba\xc3".to_vec();
        post.extra = gbk_text;

        chain.process(&mut post);

        assert!(post.get_file_data.is_none());
    }

    #[test]
    fn test_chain_knock() {
        let chain = build_default_chain();
        let mut post = Post::new("192.168.1.100");
        post.cmd_id = IPMSG_KNOCK;

        chain.process(&mut post);

        assert_eq!(post.contents.len(), 1);
        assert!(post.contents[0].is_knock());
    }

    #[test]
    fn test_br_entry_with_pubkey() {
        let chain = build_default_chain();
        let mut post = Post::new("10.0.0.1");
        post.cmd_id = IPMSG_BR_ENTRY;
        post.from.version = "feiq_plus_plus#128#MAC#0#0#0#1#9".into();
        let gbk_name = b"\x41\x6c\x69\x63\x65".to_vec();
        let mut extra = gbk_name.clone();
        extra.push(0x00);
        extra.extend_from_slice(&[1u8; 32]);
        post.extra = extra;
        chain.process(&mut post);
        assert_eq!(post.from.name, "Alice");
        assert_eq!(post.from.public_key.len(), 32);
        assert_eq!(post.from.public_key, vec![1u8; 32]);
    }

    #[test]
    fn test_ans_entry_with_pubkey() {
        let chain = build_default_chain();
        let mut post = Post::new("10.0.0.2");
        post.cmd_id = IPMSG_ANSENTRY;
        post.from.version = "feiq_plus_plus#128#MAC#0#0#0#1#9".into();
        let gbk_name = b"\x44\x61\x76\x65".to_vec();
        let mut extra = gbk_name.clone();
        extra.push(0x00);
        extra.extend_from_slice(&[3u8; 32]);
        post.extra = extra;
        chain.process(&mut post);
        assert_eq!(post.from.name, "Dave");
        assert_eq!(post.from.public_key.len(), 32);
        assert_eq!(post.from.public_key[0], 3);
    }

    #[test]
    fn test_sealed_message() {
        let chain = build_default_chain();
        let mut post = Post::new("10.0.0.1");
        post.cmd_id = IPMSG_SENDMSG | IPMSG_READCHECKOPT | IPMSG_SECRETOPT | IPMSG_SENDCHECKOPT;
        let gbk_text = b"\x62\x75\x72\x6e\x20\x61\x66\x74\x65\x72\x20\x72\x65\x61\x64\x69\x6e\x67".to_vec();
        post.extra = gbk_text;
        chain.process(&mut post);
        assert_eq!(post.contents.len(), 1);
        match &post.contents[0] {
            Content::Sealed { text, .. } => assert_eq!(text, "burn after reading"),
            other => panic!("Expected Sealed content, got: {:?}", other.content_type()),
        }
    }

    #[test]
    fn test_normal_message_with_readcheck_not_sealed() {
        let chain = build_default_chain();
        let mut post = Post::new("10.0.0.1");
        post.cmd_id = IPMSG_SENDMSG | IPMSG_READCHECKOPT | IPMSG_SENDCHECKOPT;
        let gbk_text = b"\x48\x65\x6c\x6c\x6f".to_vec();
        post.extra = gbk_text;
        chain.process(&mut post);
        assert_eq!(post.contents.len(), 1);
        match &post.contents[0] {
            Content::Text { text, .. } => assert_eq!(text, "Hello"),
            other => panic!("Expected Text content, got: {:?}", other.content_type()),
        }
    }

    // ─── File notification edge-case tests ──────────────────────

    #[test]
    fn test_chain_file_many_entries() {
        // Single notification with 15 file entries — all must be parsed correctly
        let chain = build_default_chain();
        let mut post = Post::new("192.168.1.100");
        post.cmd_id = IPMSG_SENDMSG | IPMSG_FILEATTACHOPT;

        let mut extra = vec![MSG_NULL];
        for i in 1..=15u64 {
            let entry = format!(
                "{}:file_{}.dat:{:X}:{:X}:1:",
                i,
                i,
                1024 * i,
                1700000000u64,
            );
            extra.extend_from_slice(entry.as_bytes());
            extra.push(FILELIST_SEPARATOR);
        }
        post.extra = extra;

        chain.process(&mut post);

        assert_eq!(
            post.contents.len(),
            15,
            "all 15 file entries must be parsed"
        );
        for (idx, content) in post.contents.iter().enumerate() {
            match content {
                Content::File(fc) => {
                    let n = idx + 1;
                    assert_eq!(fc.file_id, n as u64, "file_id at index {idx}");
                    assert_eq!(fc.filename, format!("file_{n}.dat"), "filename at index {idx}");
                    assert_eq!(fc.size, (1024 * n) as i64, "size at index {idx}");
                    assert_eq!(fc.modify_time, 1700000000, "mtime at index {idx}");
                    assert_eq!(fc.file_type, 1, "file_type at index {idx}");
                }
                other => {
                    panic!(
                        "Expected File at index {idx}, got {:?}",
                        other.content_type()
                    )
                }
            }
        }
    }

    #[test]
    fn test_chain_file_duplicate_ids() {
        // Multiple file entries with the same file_id — all must be parsed and preserved.
        // File ID duplication is allowed: different files can share a protocol ID
        // when the sender groups them (e.g. directory listing or retransmission).
        let chain = build_default_chain();
        let mut post = Post::new("192.168.1.200");
        post.cmd_id = IPMSG_SENDMSG | IPMSG_FILEATTACHOPT;

        let mut extra = vec![MSG_NULL];
        for (name, size) in [("a.txt", 100u64), ("b.txt", 200), ("c.txt", 300)] {
            let entry = format!("42:{name}:{size:X}:{:X}:1:", 1700000000u64);
            extra.extend_from_slice(entry.as_bytes());
            extra.push(FILELIST_SEPARATOR);
        }
        post.extra = extra;

        chain.process(&mut post);

        assert_eq!(post.contents.len(), 3, "all 3 entries parsed");
        let expected = [("a.txt", 100i64), ("b.txt", 200), ("c.txt", 300)];
        for (idx, (content, (exp_name, exp_size))) in
            post.contents.iter().zip(expected.iter()).enumerate()
        {
            match content {
                Content::File(fc) => {
                    assert_eq!(fc.file_id, 42, "file_id at index {idx}");
                    assert_eq!(fc.filename, *exp_name, "filename at index {idx}");
                    assert_eq!(fc.size, *exp_size, "size at index {idx}");
                    assert_eq!(fc.modify_time, 1700000000);
                    assert_eq!(fc.file_type, 1);
                }
                other => {
                    panic!(
                        "Expected File at index {idx}, got {:?}",
                        other.content_type()
                    )
                }
            }
        }
    }

    #[test]
    fn test_chain_file_mixed_regular_directory() {
        // Notification mixing regular files (file_type=1) and directories (file_type=2).
        // Each entry's file_type must be preserved exactly.
        let chain = build_default_chain();
        let mut post = Post::new("10.0.0.50");
        post.cmd_id = IPMSG_SENDMSG | IPMSG_FILEATTACHOPT;

        let mut extra = vec![MSG_NULL];
        for (id, name, size, ftype) in [
            (1u64, "doc.pdf", 0x8000u64, 1u32),       // regular
            (2, "my_folder", 0, 2u32),                  // directory
            (3, "photo.jpg", 0x10000u64, 1u32),         // regular
            (4, "subdir", 0, 2u32),                      // directory
        ] {
            let entry =
                format!("{id}:{name}:{size:X}:{:X}:{ftype:X}:", 0xA5A5A5u64);
            extra.extend_from_slice(entry.as_bytes());
            extra.push(FILELIST_SEPARATOR);
        }
        post.extra = extra;

        chain.process(&mut post);

        assert_eq!(post.contents.len(), 4, "4 entries parsed");

        let expected_types = [1u32, 2, 1, 2];
        let expected_names = ["doc.pdf", "my_folder", "photo.jpg", "subdir"];
        for (idx, content) in post.contents.iter().enumerate() {
            match content {
                Content::File(fc) => {
                    assert_eq!(
                        fc.file_type, expected_types[idx],
                        "file_type at index {idx}"
                    );
                    assert_eq!(
                        fc.filename, expected_names[idx],
                        "filename at index {idx}"
                    );
                    assert_eq!(fc.file_id, (idx + 1) as u64, "file_id at index {idx}");
                    if idx == 0 {
                        assert_eq!(fc.size, 0x8000);
                    } else if idx == 2 {
                        assert_eq!(fc.size, 0x10000);
                    }
                }
                other => {
                    panic!(
                        "Expected File at index {idx}, got {:?}",
                        other.content_type()
                    )
                }
            }
        }
    }

    #[test]
    fn test_chain_file_separator_positions() {
        // FILELIST_SEPARATOR (0x07) in unexpected positions.
        // These are robustness tests: the parser must not panic or corrupt data.

        // 1. Trailing separator after the last valid entry (common in legacy clients)
        {
            let chain = build_default_chain();
            let mut post = Post::new("10.0.0.1");
            post.cmd_id = IPMSG_SENDMSG | IPMSG_FILEATTACHOPT;
            let mut extra = vec![MSG_NULL];
            extra.extend_from_slice(b"1:file.txt:100:1A:1:");
            extra.push(FILELIST_SEPARATOR); // trailing — valid, ignored after last entry
            post.extra = extra;

            chain.process(&mut post);
            assert_eq!(post.contents.len(), 1, "trailing separator: 1 entry");
            if let Content::File(fc) = &post.contents[0] {
                assert_eq!(fc.file_id, 1);
                assert_eq!(fc.filename, "file.txt");
            } else {
                panic!("Expected File");
            }
        }

        // 2. Leading separator (before any entry) — causes immediate loop break, 0 entries
        {
            let chain = build_default_chain();
            let mut post = Post::new("10.0.0.2");
            post.cmd_id = IPMSG_SENDMSG | IPMSG_FILEATTACHOPT;
            let mut extra = vec![MSG_NULL];
            extra.push(FILELIST_SEPARATOR); // leading — no entry before it
            extra.extend_from_slice(b"2:file2.txt:200:2A:1:");
            extra.push(FILELIST_SEPARATOR);
            post.extra = extra;

            chain.process(&mut post);
            assert_eq!(
                post.contents.len(),
                0,
                "leading separator: 0 entries parsed"
            );
        }

        // 3. Consecutive separators (empty entry between valid entries) — stops at the gap
        {
            let chain = build_default_chain();
            let mut post = Post::new("10.0.0.3");
            post.cmd_id = IPMSG_SENDMSG | IPMSG_FILEATTACHOPT;
            let mut extra = vec![MSG_NULL];
            extra.extend_from_slice(b"3:first.txt:100:1A:1:");
            extra.push(FILELIST_SEPARATOR);
            extra.push(FILELIST_SEPARATOR); // empty-entry gap
            extra.extend_from_slice(b"4:second.txt:200:2A:1:");
            extra.push(FILELIST_SEPARATOR);
            post.extra = extra;

            chain.process(&mut post);
            assert_eq!(
                post.contents.len(),
                1,
                "consecutive separator gap: only entry before gap"
            );
            if let Content::File(fc) = &post.contents[0] {
                assert_eq!(fc.file_id, 3);
                assert_eq!(fc.filename, "first.txt");
            } else {
                panic!("Expected File");
            }
        }

        // 4. Only null + separator (no file data at all)
        {
            let chain = build_default_chain();
            let mut post = Post::new("10.0.0.4");
            post.cmd_id = IPMSG_SENDMSG | IPMSG_FILEATTACHOPT;
            post.extra = vec![MSG_NULL, FILELIST_SEPARATOR];

            chain.process(&mut post);
            assert_eq!(post.contents.len(), 0, "null+separator only: 0 entries");
        }
    }

}
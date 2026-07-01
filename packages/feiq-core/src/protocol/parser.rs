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
            }

            start = end + 1;
            if start >= file_data.len() {
                break;
            }
        }

        false
    }
}

/// Parse a single file task from bytes
fn parse_file_task(data: &[u8], is_utf8: bool) -> Option<FileContent> {
    let values = split_allow_separator(data, HLIST_ENTRY_SEPARATOR);
    if values.len() < 5 {
        return None;
    }

    let file_id: u64 = values[0].parse().ok()?;
    let filename = decode_by_utf8opt(values[1].as_bytes(), is_utf8);
    let size: i64 = i64::from_str_radix(&values[2], 16).ok()?;
    let modify_time: i64 = i64::from_str_radix(&values[3], 16).ok()?;
    let file_type: u32 = u32::from_str_radix(&values[4], 16).ok()?;

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

        // Format: packetNo:fileId:offset:
        let extra_str = String::from_utf8_lossy(&post.extra);
        let parts: Vec<&str> = extra_str.trim_end_matches(':').split(':').collect();

        if parts.len() >= 3 {
            if let (Ok(packet_no), Ok(file_id), Ok(offset)) = (
                parts[0].parse::<u64>(),
                parts[1].parse::<u64>(),
                parts[2].parse::<i64>(),
            ) {
                post.get_file_data = Some(GetFileData {
                    packet_no,
                    file_id,
                    offset,
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
                });
            }
        }

        true // stop chain
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
    chain.add_handler(Box::new(RecvBrExit));
    chain.add_handler(Box::new(RecvSendCheck));
    chain.add_handler(Box::new(RecvReadCheck));
    chain.add_handler(Box::new(RecvReadMessage));
    chain.add_handler(Box::new(RecvText));
    chain.add_handler(Box::new(RecvImage));
    chain.add_handler(Box::new(RecvKnock));
    chain.add_handler(Box::new(RecvFile));
    chain.add_handler(Box::new(RecvGetFileData));
    chain.add_handler(Box::new(RecvReleaseFiles));
    chain.add_handler(Box::new(EndRecv));

    chain
}

#[cfg(test)]
mod tests {
    use super::*;

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
}

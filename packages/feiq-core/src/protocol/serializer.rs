//! Message serializer: pack IPMSG messages for sending, parse for receiving
//! Mirrors feiqcommu.cpp: pack(), dumpRaw(), dumpVersionInfo()

use crate::protocol::constants::*;
use crate::protocol::types::*;

use std::io::Write;

/// Trait for types that can write their body into an outgoing message
pub trait SendProtocol {
    fn cmd_id(&self) -> u32;
    fn write_body(&self, buf: &mut Vec<u8>);
}

// ─── Packing ─────────────────────────────────────────────────

/// Pack a message into the wire format:
/// `version:packetNo:senderName:hostName:cmdId:body_bytes\0`
pub fn pack_message(
    packet_no: u64,
    sender_name: &str,
    host_name: &str,
    version: &str,
    cmd_id: u32,
    body_bytes: &[u8],
) -> Vec<u8> {
    let mut buf = Vec::with_capacity(256 + body_bytes.len());

    // Header: version:packetNo:senderName:hostName:cmdId:
    write!(buf, "{}:", version).unwrap();
    write!(buf, "{}:", packet_no).unwrap();

    // Replace ':' in name with HOSTLIST_DUMMY to avoid breaking the protocol
    let safe_name = sender_name.replace(':', &String::from_utf8_lossy(&[HOSTLIST_DUMMY]));
    write!(buf, "{}:", safe_name).unwrap();

    let safe_host = host_name.replace(':', &String::from_utf8_lossy(&[HOSTLIST_DUMMY]));
    write!(buf, "{}:", safe_host).unwrap();

    write!(buf, "{}:", cmd_id).unwrap();

    // Body
    buf.extend_from_slice(body_bytes);
    buf.push(MSG_NULL);

    buf
}

/// Pack using a SendProtocol trait object (for text/knock/file content senders)
pub fn pack_send(
    sender: &dyn SendProtocol,
    packet_no: u64,
    sender_name: &str,
    host_name: &str,
    version: &str,
) -> Vec<u8> {
    let mut body = Vec::new();
    sender.write_body(&mut body);
    pack_message(packet_no, sender_name, host_name, version, sender.cmd_id(), &body)
}

// ─── Parsing ─────────────────────────────────────────────────

/// Represents the parsed state of a raw message during protocol chain processing
pub struct ParseState {
    /// The post being built
    pub post: Post,
    /// Whether parsing completed successfully
    pub valid: bool,
}

/// Parse raw IPMSG datagram into a Post struct.
/// Returns None if the datagram is malformed or self-filtered.
///
/// This mirrors the logic in feiqcommu.cpp: onRecv() -> dumpRaw() -> dumpVersionInfo() -> self-filter.
pub fn parse_raw(
    data: &[u8],
    sender_ip: &str,
    self_mac: &str,
    self_name: &str,
) -> Option<Post> {
    // Remove trailing null bytes
    let data = strip_trailing_nulls(data);
    if data.is_empty() {
        return None;
    }

    // Split by HLIST_ENTRY_SEPARATOR (':') for the first 5 fields
    let (header_fields, extra) = split_header(&data);

    if header_fields.len() < 5 {
        tracing::debug!("Malformed packet: less than 5 header fields");
        return None;
    }

    let version = &header_fields[0];
    let packet_no = &header_fields[1];
    // Field 2 (sender_name) and 3 (host_name) have HOSTLIST_DUMMY -> ':'
    let sender_name = header_fields[2].replace(
        char::from(HOSTLIST_DUMMY),
        &String::from_utf8_lossy(&[HLIST_ENTRY_SEPARATOR]),
    );
    let host_name = header_fields[3].replace(
        char::from(HOSTLIST_DUMMY),
        &String::from_utf8_lossy(&[HLIST_ENTRY_SEPARATOR]),
    );
    let cmd_id: u32 = header_fields[4].parse().unwrap_or(0);

    // Extract MAC from version string (3rd #-separated segment)
    let version_info = parse_version_info(version);

    let mut post = Post::new(sender_ip);
    post.packet_no = packet_no.to_string();
    post.cmd_id = cmd_id;
    post.from.pc_name = sender_name.clone();
    post.from.host = host_name;
    post.from.version = version.to_string();
    post.from.mac = version_info.mac.clone();
    post.extra = extra;

    // Unless receiving an exit message, assume online
    if !is_cmd_set(cmd_id, IPMSG_BR_EXIT) {
        post.from.online = true;
    }

    // Self-message filter: drop if MAC matches AND name matches
    if !version_info.mac.is_empty()
        && !self_mac.is_empty()
        && version_info.mac == self_mac
        && sender_name == self_name
    {
        tracing::trace!("Self-message filtered: ip={sender_ip}, mac={}", version_info.mac);
        return None;
    }

    Some(post)
}

/// Parse version string like "1_lbt6_0#128#MAC#0#0#0#4001#9"
/// Returns VersionInfo with the MAC address extracted (3rd segment)
pub fn parse_version_info(version: &str) -> VersionInfo {
    let parts: Vec<&str> = version.split('#').collect();
    VersionInfo {
        mac: parts.get(2).copied().unwrap_or("").to_string(),
        version: version.to_string(),
    }
}

// ─── Helper functions ────────────────────────────────────────

/// Split raw data into first 5 header fields (':' separated) and remaining extra bytes
fn split_header(data: &[u8]) -> (Vec<String>, Vec<u8>) {
    let mut fields = Vec::new();
    let mut pos = 0;
    let len = data.len();

    while fields.len() < 5 && pos < len {
        let sep_pos = data[pos..]
            .iter()
            .position(|&b| b == HLIST_ENTRY_SEPARATOR)
            .map(|p| pos + p);

        match sep_pos {
            Some(p) => {
                let field_bytes = &data[pos..p];
                let field = String::from_utf8_lossy(field_bytes).into_owned();
                fields.push(field);
                pos = p + 1;
            }
            None => {
                // Last field (no trailing ':')
                let field_bytes = &data[pos..];
                let field = String::from_utf8_lossy(field_bytes).into_owned();
                fields.push(field);
                pos = len;
            }
        }
    }

    let extra = if pos < len {
        data[pos..].to_vec()
    } else {
        Vec::new()
    };

    // Remove trailing null from extra if present
    let extra = strip_trailing_nulls_vec(extra);

    (fields, extra)
}

fn strip_trailing_nulls(data: &[u8]) -> &[u8] {
    let end = data.iter().rposition(|&b| b != 0).map_or(0, |p| p + 1);
    &data[..end]
}

fn strip_trailing_nulls_vec(mut data: Vec<u8>) -> Vec<u8> {
    while data.last() == Some(&0) {
        data.pop();
    }
    data
}

// ─── Helper: split with escaped separator (::) ───────────────

/// Split bytes by separator, treating double-separator (::) as escaped single
pub fn split_allow_separator(data: &[u8], sep: u8) -> Vec<String> {
    let mut values = Vec::new();
    let mut current = Vec::new();
    let mut i = 0;

    while i < data.len() {
        if data[i] == sep {
            // Check if next byte is also sep (escaped)
            if i + 1 < data.len() && data[i + 1] == sep {
                current.push(sep);
                i += 2;
            } else {
                values.push(String::from_utf8_lossy(&current).into_owned());
                current.clear();
                i += 1;
            }
        } else {
            current.push(data[i]);
            i += 1;
        }
    }

    if !current.is_empty() {
        values.push(String::from_utf8_lossy(&current).into_owned());
    }

    values
}

/// Extract filename from a file path
pub fn get_filename_from_path(path: &str) -> String {
    let path = path.trim_end_matches('/');
    match path.rfind('/') {
        Some(pos) => path[pos + 1..].to_string(),
        None => path.to_string(),
    }
}

/// Check if string starts with pattern
pub fn starts_with(s: &str, pattern: &str) -> bool {
    s.len() >= pattern.len() && &s[..pattern.len()] == pattern
}

/// Check if string ends with pattern
pub fn ends_with(s: &str, pattern: &str) -> bool {
    s.len() >= pattern.len() && &s[s.len() - pattern.len()..] == pattern
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_header_basic() {
        // Simulate: "1:100:Alice:myhost:32:Hello\0"
        let data = b"1:100:Alice:myhost:32:Hello\0".to_vec();
        let (fields, extra) = split_header(&data);
        assert_eq!(fields.len(), 5); // Only first 5 fields extracted
        assert_eq!(fields[0], "1");
        assert_eq!(fields[1], "100");
        assert_eq!(fields[2], "Alice");
        assert_eq!(fields[3], "myhost");
        assert_eq!(fields[4], "32");
        assert_eq!(extra, b"Hello");
    }

    #[test]
    fn test_parse_raw_self_filter() {
        let version = "1_lbt6_0#128#AABBCCDDEEFF#0#0#0#4001#9";
        let name = "TestUser";
        let packet = format!("{version}:100:{name}:host:32:X\0");

        // Should filter out self
        let result = parse_raw(
            packet.as_bytes(),
            "192.168.1.1",
            "AABBCCDDEEFF",
            name,
        );
        assert!(result.is_none());
    }

    #[test]
    fn test_split_allow_separator_escaped() {
        // "file1::txt:1024" -> ["file1:txt", "1024"]
        let data = b"file1::txt:1024".to_vec();
        let values = split_allow_separator(&data, b':');
        assert_eq!(values.len(), 2);
        assert_eq!(values[0], "file1:txt");
        assert_eq!(values[1], "1024");
    }

    #[test]
    fn test_get_filename_from_path() {
        assert_eq!(get_filename_from_path("/home/user/file.txt"), "file.txt");
        assert_eq!(get_filename_from_path("readme.md"), "readme.md");
        assert_eq!(get_filename_from_path("/path/to/dir/"), "dir");
    }

    #[test]
    fn test_parse_version_info() {
        let version = "1_lbt6_0#128#AABBCCDDEEFF#0#0#0#4001#9";
        let info = parse_version_info(version);
        assert_eq!(info.mac, "AABBCCDDEEFF");
    }
}

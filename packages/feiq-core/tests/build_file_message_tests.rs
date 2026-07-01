//! Tests for `build_file_message` filename encoding.
//!
//! Verifies:
//! - GBK encoding for legacy peers vs UTF-8 for feiq++ peers
//! - Colon escaping (`:` -> `::`)
//! - Lossy character replacement (emoji -> `?`)
//! - Roundtrip through encode_gbk / decode_gbk
//! - Command flag bits (UTF8OPT, FILEATTACHOPT, SENDMSG)

use feiq_core::engine::engine::build_file_message;
use feiq_core::protocol::constants::{
    IPMSG_FILEATTACHOPT, IPMSG_FILE_REGULAR, IPMSG_SENDMSG, IPMSG_UTF8OPT,
};
use feiq_core::protocol::encoding::{decode_gbk, encode_gbk};
use feiq_core::protocol::types::FileContent;

// ─── Helpers ──────────────────────────────────────────────────

/// Check that the filename portion inside `build_file_message` output is correct.
/// The packed message has format:
///   version:packetNo:name:host:cmdId:\0{file_id}:{filename}:{size:X}:{modify_time:X}:{file_type:X}:\x07\0
fn check_filename_in_message(
    msg: &[u8],
    file_id: u64,
    expected_filename_bytes: &[u8],
    size: i64,
    modify_time: i64,
    file_type: u32,
) {
    // First null byte is the body's leading MSG_NULL
    let first_null = msg.iter().position(|&b| b == 0).expect("no null byte");
    let after_body_null = first_null + 1;
    let id_label = format!("{}:", file_id);
    let id_bytes = id_label.as_bytes();
    assert_eq!(
        &msg[after_body_null..after_body_null + id_bytes.len()],
        id_bytes,
        "file_id label mismatch"
    );
    let filename_start = after_body_null + id_bytes.len();
    assert_eq!(
        &msg[filename_start..filename_start + expected_filename_bytes.len()],
        expected_filename_bytes,
        "filename bytes mismatch"
    );
    let suffix = format!(":{:X}:{:X}:{:X}:\x07", size, modify_time, file_type);
    let suffix_bytes = suffix.as_bytes();
    let after_filename = filename_start + expected_filename_bytes.len();
    assert_eq!(
        &msg[after_filename..after_filename + suffix_bytes.len()],
        suffix_bytes,
        "suffix mismatch"
    );
}

fn first_null(msg: &[u8]) -> usize {
    msg.iter().position(|&b| b == 0).unwrap()
}

fn make_content(filename: &str) -> FileContent {
    FileContent {
        file_id: 0,
        filename: filename.into(),
        path: String::new(),
        size: 100,
        modify_time: 200,
        file_type: IPMSG_FILE_REGULAR,
        packet_no: 0,
        local_task_id: None,
    }
}

// ─── GBK Chinese Characters ────────────────────────────────────

#[test]
fn test_gbk_chinese_feiqpp() {
    let content = make_content("中文文件名称.txt");
    let safe = content.filename.replace(':', "::");
    let msg = build_file_message(100, "Tester", "Host", "v1", &content, true);
    check_filename_in_message(&msg, 0, safe.as_bytes(), 100, 200, IPMSG_FILE_REGULAR);
}

#[test]
fn test_gbk_chinese_legacy() {
    let content = make_content("中文文件名称.txt");
    let safe = content.filename.replace(':', "::");
    let gbk_bytes = encode_gbk(&safe);
    let msg = build_file_message(100, "Tester", "Host", "v1", &content, false);
    check_filename_in_message(&msg, 0, &gbk_bytes, 100, 200, IPMSG_FILE_REGULAR);
    assert_ne!(gbk_bytes, safe.as_bytes(), "GBK should differ from UTF-8 for Chinese");
}

// ─── Mixed ASCII + GBK ─────────────────────────────────────────

#[test]
fn test_mixed_ascii_gbk_feiqpp() {
    let content = make_content("screenshot(截图)_v2.png");
    let safe = content.filename.replace(':', "::");
    let msg = build_file_message(200, "Tester", "Host", "v1", &content, true);
    check_filename_in_message(&msg, 0, safe.as_bytes(), 100, 200, IPMSG_FILE_REGULAR);
}

#[test]
fn test_mixed_ascii_gbk_legacy() {
    let content = make_content("screenshot(截图)_v2.png");
    let safe = content.filename.replace(':', "::");
    let gbk_bytes = encode_gbk(&safe);
    let msg = build_file_message(200, "Tester", "Host", "v1", &content, false);
    check_filename_in_message(&msg, 0, &gbk_bytes, 100, 200, IPMSG_FILE_REGULAR);
}

// ─── Lossy Characters (emoji, extended Unicode) ────────────────

#[test]
fn test_lossy_chars_feiqpp() {
    // feiq++ mode: emoji preserved as UTF-8
    let content = make_content("emoji😀test🎉.txt");
    let safe = content.filename.replace(':', "::");
    let msg = build_file_message(300, "Tester", "Host", "v1", &content, true);
    check_filename_in_message(&msg, 0, safe.as_bytes(), 100, 200, IPMSG_FILE_REGULAR);
}

#[test]
fn test_lossy_chars_legacy() {
    // Legacy mode: GBK uses HTML NCR (&#N;) for unencodable chars
    let content = make_content("emoji😀test🎉.txt");
    let safe = content.filename.replace(':', "::");
    let gbk_bytes = encode_gbk(&safe);
    let msg = build_file_message(300, "Tester", "Host", "v1", &content, false);
    check_filename_in_message(&msg, 0, &gbk_bytes, 100, 200, IPMSG_FILE_REGULAR);
    assert_ne!(gbk_bytes, safe.as_bytes(), "GBK should differ from UTF-8 for emoji");
    // NCR starts with '&#' — the emoji is replaced by NCR, not a single byte
    assert!(gbk_bytes.windows(2).any(|w| w == b"&#"), "GBK uses NCR for unencodable chars");
}

#[test]
fn test_lossy_chars_roundtrip() {
    let filename = "readme😀notes.txt";
    let safe = filename.replace(':', "::");
    let encoded = encode_gbk(&safe);
    let decoded = decode_gbk(&encoded);
    assert!(decoded.contains("readme"), "ASCII prefix preserved");
    assert!(decoded.contains("notes.txt"), "ASCII suffix preserved");
    // encoding_rs GBK encodes unencodable chars as HTML NCR (&#N;)
    assert!(decoded.contains("&#"), "NCR entity present in roundtrip output");
    assert!(!decoded.contains('😀'), "Emoji lost in lossy roundtrip");
}

// ─── Colon Replacement ─────────────────────────────────────────

#[test]
fn test_colon_replacement_feiqpp() {
    let content = make_content("file:name:test.txt");
    let safe = content.filename.replace(':', "::");
    assert_eq!(safe, "file::name::test.txt", "colon escaping");
    let msg = build_file_message(400, "Tester", "Host", "v1", &content, true);
    check_filename_in_message(&msg, 0, safe.as_bytes(), 100, 200, IPMSG_FILE_REGULAR);
}

#[test]
fn test_colon_replacement_legacy() {
    let content = make_content("file:name:test.txt");
    let safe = content.filename.replace(':', "::");
    let gbk_bytes = encode_gbk(&safe);
    let msg = build_file_message(400, "Tester", "Host", "v1", &content, false);
    check_filename_in_message(&msg, 0, &gbk_bytes, 100, 200, IPMSG_FILE_REGULAR);
}

#[test]
fn test_colon_only_filename() {
    let content = make_content(":");
    let safe = content.filename.replace(':', "::");
    assert_eq!(safe, "::", "single colon doubled");
    let msg = build_file_message(500, "Tester", "Host", "v1", &content, true);
    check_filename_in_message(&msg, 0, safe.as_bytes(), 100, 200, IPMSG_FILE_REGULAR);
}

#[test]
fn test_colon_only_filename_legacy() {
    let content = make_content(":");
    let safe = content.filename.replace(':', "::");
    let gbk_bytes = encode_gbk(&safe);
    let msg = build_file_message(500, "Tester", "Host", "v1", &content, false);
    check_filename_in_message(&msg, 0, &gbk_bytes, 100, 200, IPMSG_FILE_REGULAR);
}

// ─── Edge Cases ────────────────────────────────────────────────

#[test]
fn test_empty_filename() {
    let content = make_content("");
    let msg_pp = build_file_message(600, "Tester", "Host", "v1", &content, true);
    check_filename_in_message(&msg_pp, 0, b"", 100, 200, IPMSG_FILE_REGULAR);
    let msg_legacy = build_file_message(600, "Tester", "Host", "v1", &content, false);
    check_filename_in_message(&msg_legacy, 0, b"", 100, 200, IPMSG_FILE_REGULAR);
}

#[test]
fn test_pure_ascii() {
    let content = make_content("document (1).pdf");
    let safe = content.filename.replace(':', "::");
    let msg_pp = build_file_message(700, "Tester", "Host", "v1", &content, true);
    check_filename_in_message(&msg_pp, 0, safe.as_bytes(), 100, 200, IPMSG_FILE_REGULAR);
    let msg_legacy = build_file_message(700, "Tester", "Host", "v1", &content, false);
    let gbk_bytes = encode_gbk(&safe);
    assert_eq!(gbk_bytes, safe.as_bytes(), "GBK of ASCII equals ASCII");
    check_filename_in_message(&msg_legacy, 0, &gbk_bytes, 100, 200, IPMSG_FILE_REGULAR);
}

#[test]
fn test_cmd_bits_feiqpp() {
    let content = make_content("test.bin");
    let msg = build_file_message(1, "Tester", "Host", "v1", &content, true);
    let header_end = first_null(&msg);
    let header = String::from_utf8_lossy(&msg[..header_end]);
    // Header ends with ":<cmd>:" — rsplit gives ["", "<cmd>", ...]
    let cmd_str = header.rsplit(':').nth(1).expect("cmd field");
    let cmd: u32 = cmd_str.parse().expect("cmd as u32");
    assert!(cmd & IPMSG_UTF8OPT != 0, "feiq++ cmd must include UTF8OPT");
    assert!(cmd & IPMSG_FILEATTACHOPT != 0, "cmd must include FILEATTACHOPT");
    assert!(cmd & IPMSG_SENDMSG != 0, "cmd must include SENDMSG");
}

#[test]
fn test_cmd_bits_legacy() {
    let content = make_content("test.bin");
    let msg = build_file_message(1, "Tester", "Host", "v1", &content, false);
    let header_end = first_null(&msg);
    let header = String::from_utf8_lossy(&msg[..header_end]);
    let cmd_str = header.rsplit(':').nth(1).expect("cmd field");
    let cmd: u32 = cmd_str.parse().expect("cmd as u32");
    assert!(cmd & IPMSG_UTF8OPT == 0, "legacy cmd must NOT include UTF8OPT");
    assert!(cmd & IPMSG_FILEATTACHOPT != 0, "legacy cmd must include FILEATTACHOPT");
    assert!(cmd & IPMSG_SENDMSG != 0, "legacy cmd must include SENDMSG");
}

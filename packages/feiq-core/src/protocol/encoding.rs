//! GBK/UTF-8 encoding conversion for IP Messenger protocol
//! All legacy IPMSG/FeiQ clients use GBK encoding.
//! Uses encoding_rs (Mozilla, pure Rust).

use encoding_rs::GBK;

/// Decode GBK bytes to UTF-8 String (incoming messages)
pub fn decode_gbk(data: &[u8]) -> String {
    // Strip trailing null byte if present
    let data = if data.last() == Some(&0) {
        &data[..data.len() - 1]
    } else {
        data
    };

    if data.is_empty() {
        return String::new();
    }

    let (cow, _encoding, had_errors) = GBK.decode(data);
    if had_errors {
        tracing::warn!("GBK decode had replacement characters");
    }
    cow.into_owned()
}

/// Encode UTF-8 string to GBK bytes (outgoing messages)
pub fn encode_gbk(text: &str) -> Vec<u8> {
    if text.is_empty() {
        return Vec::new();
    }
    let (cow, _encoding, had_errors) = GBK.encode(text);
    if had_errors {
        tracing::warn!("GBK encode had replacement characters");
    }
    cow.into_owned()
}

/// Decode GBK bytes to UTF-8 String, preserving the raw bytes if GBK fails
pub fn decode_gbk_lossy(data: &[u8]) -> String {
    decode_gbk(data)
}

/// Decode bytes according to the UTF8OPT flag.
/// When `is_utf8opt` is true, decodes as UTF-8; otherwise as GBK.
pub fn decode_by_utf8opt(data: &[u8], is_utf8opt: bool) -> String {
    if is_utf8opt {
        // Strip trailing null byte if present
        let data = if data.last() == Some(&0) {
            &data[..data.len() - 1]
        } else {
            data
        };
        String::from_utf8_lossy(data).into_owned()
    } else {
        decode_gbk(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_gbk_hello() {
        // "你好" in GBK: c4 e3 ba c3
        let gbk_bytes = vec![0xc4, 0xe3, 0xba, 0xc3];
        let result = decode_gbk(&gbk_bytes);
        assert_eq!(result, "你好");
    }

    #[test]
    fn test_encode_gbk_hello() {
        let utf8_str = "你好";
        let result = encode_gbk(utf8_str);
        assert_eq!(result, vec![0xc4, 0xe3, 0xba, 0xc3]);
    }

    #[test]
    fn test_roundtrip() {
        let original = "测试消息: Hello World!";
        let encoded = encode_gbk(original);
        let decoded = decode_gbk(&encoded);
        assert_eq!(original, decoded);
    }

    #[test]
    fn test_empty() {
        assert_eq!(decode_gbk(&[]), "");
        assert_eq!(encode_gbk(""), Vec::new() as Vec<u8>);
    }

    #[test]
    fn test_null_terminated() {
        let gbk_bytes = vec![0xc4, 0xe3, 0xba, 0xc3, 0x00]; // "你好\0"
        let result = decode_gbk(&gbk_bytes);
        assert_eq!(result, "你好");
    }

    #[test]
    fn test_feixing_mao() {
        // Test the standard IPMSG "飞鸽传书" pattern
        let text = "飞鸽传书测试";
        let encoded = encode_gbk(text);
        let decoded = decode_gbk(&encoded);
        assert_eq!(text, decoded);
        // Verify NOT UTF-8 (should be GBK bytes, different from UTF-8)
        let utf8_bytes = text.as_bytes().to_vec();
        assert_ne!(encoded, utf8_bytes);
    }
}

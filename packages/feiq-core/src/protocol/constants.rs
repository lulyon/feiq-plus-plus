//! IP Messenger (飞鸽传书) Protocol Constants
//! Based on IPMSG Draft-9 (1996-2003) and FeiQ extensions
//! Reference: /Users/zhihu/code/feiq/feiqlib/ipmsg.h

/// Default UDP/TCP port for IP Messenger protocol
pub const IPMSG_PORT: u16 = 2425;

/// Maximum UDP receive buffer size
pub const MAX_RCV_SIZE: usize = 4096;

// ─── Commands (low 8 bits) ───────────────────────────────────

pub const IPMSG_NOOPERATION: u32 = 0x00000000;
pub const IPMSG_BR_ENTRY: u32 = 0x00000001; // user online broadcast
pub const IPMSG_BR_EXIT: u32 = 0x00000002; // user offline broadcast
pub const IPMSG_ANSENTRY: u32 = 0x00000003; // reply to online broadcast
pub const IPMSG_BR_ABSENCE: u32 = 0x00000004; // absence mode / name change

pub const IPMSG_BR_ISGETLIST: u32 = 0x00000010;
pub const IPMSG_OKGETLIST: u32 = 0x00000011;
pub const IPMSG_GETLIST: u32 = 0x00000012;
pub const IPMSG_ANSLIST: u32 = 0x00000013;
pub const IPMSG_BR_ISGETLIST2: u32 = 0x00000018;

pub const IPMSG_SENDMSG: u32 = 0x00000020; // send message
pub const IPMSG_RECVMSG: u32 = 0x00000021; // confirm message received
pub const IPMSG_READMSG: u32 = 0x00000030; // message read (sealed)
pub const IPMSG_DELMSG: u32 = 0x00000031;
pub const IPMSG_ANSREADMSG: u32 = 0x00000032; // confirm read (v8+)

pub const IPMSG_GETINFO: u32 = 0x00000040; // request protocol version
pub const IPMSG_SENDINFO: u32 = 0x00000041; // send protocol version

pub const IPMSG_GETABSENCEINFO: u32 = 0x00000050; // ask if away
pub const IPMSG_SENDABSENCEINFO: u32 = 0x00000051; // reply away status

pub const IPMSG_GETFILEDATA: u32 = 0x00000060; // request file data (TCP)
pub const IPMSG_RELEASEFILES: u32 = 0x00000061; // release files
pub const IPMSG_GETDIRFILES: u32 = 0x00000062; // request directory files (TCP)

pub const IPMSG_GETPUBKEY: u32 = 0x00000072; // request RSA public key
pub const IPMSG_ANSPUBKEY: u32 = 0x00000073; // reply RSA public key

// FeiQ protocol extensions
pub const IPMSG_OPEN_YOU: u32 = 0x00000077; // open chat window
pub const IPMSG_INPUTING: u32 = 0x00000079; // typing indicator
pub const IPMSG_INPUT_END: u32 = 0x0000007A; // typing ended
pub const IPMSG_SENDIMAGE: u32 = 0x000000C0; // send image (8-char ID)
pub const IPMSG_KNOCK: u32 = 0x000000D1; // window shake

// feiq++ custom extension: image via file transfer channel
pub const IPMSG_SENDIMAGE_DATA: u32 = 0x000000C1;

// ─── Options (high 24 bits, for all commands) ───────────────

pub const IPMSG_ABSENCEOPT: u32 = 0x00000100;
pub const IPMSG_SERVEROPT: u32 = 0x00000200;
pub const IPMSG_DIALUPOPT: u32 = 0x00010000;
pub const IPMSG_FILEATTACHOPT: u32 = 0x00200000;
pub const IPMSG_ENCRYPTOPT: u32 = 0x00400000;
pub const IPMSG_UTF8OPT: u32 = 0x00800000;

// ─── Options for send command ────────────────────────────────

pub const IPMSG_SENDCHECKOPT: u32 = 0x00000100;
pub const IPMSG_SECRETOPT: u32 = 0x00000200;
pub const IPMSG_BROADCASTOPT: u32 = 0x00000400;
pub const IPMSG_MULTICASTOPT: u32 = 0x00000800;
pub const IPMSG_NOPOPUPOPT: u32 = 0x00001000;
pub const IPMSG_AUTORETOPT: u32 = 0x00002000;
pub const IPMSG_RETRYOPT: u32 = 0x00004000;
pub const IPMSG_PASSWORDOPT: u32 = 0x00008000;
pub const IPMSG_NOLOGOPT: u32 = 0x00020000;
pub const IPMSG_NEWMUTIOPT: u32 = 0x00040000;
pub const IPMSG_NOADDLISTOPT: u32 = 0x00080000;
pub const IPMSG_READCHECKOPT: u32 = 0x00100000;
pub const IPMSG_SECRETEXOPT: u32 = IPMSG_READCHECKOPT | IPMSG_SECRETOPT;

/// Options that indicate no reply is needed
pub const IPMSG_NO_REPLY_OPTS: u32 = IPMSG_BROADCASTOPT | IPMSG_AUTORETOPT;

// ─── Encryption flags (v9) ───────────────────────────────────

pub const IPMSG_RSA_512: u32 = 0x00000001;
pub const IPMSG_RSA_1024: u32 = 0x00000002;
pub const IPMSG_RSA_2048: u32 = 0x00000004;
pub const IPMSG_RC2_40: u32 = 0x00001000;
pub const IPMSG_RC2_128: u32 = 0x00004000;
pub const IPMSG_RC2_256: u32 = 0x00008000;
pub const IPMSG_BLOWFISH_128: u32 = 0x00020000;
pub const IPMSG_BLOWFISH_256: u32 = 0x00040000;
pub const IPMSG_AES_128: u32 = 0x00100000;
pub const IPMSG_AES_192: u32 = 0x00200000;
pub const IPMSG_AES_256: u32 = 0x00400000;
pub const IPMSG_SIGN_STAMPOPT: u32 = 0x01000000;
pub const IPMSG_SIGN_MD5: u32 = 0x10000000;
pub const IPMSG_SIGN_SHA1: u32 = 0x20000000;

// ─── File types ──────────────────────────────────────────────

pub const IPMSG_FILE_REGULAR: u32 = 0x00000001;
pub const IPMSG_FILE_DIR: u32 = 0x00000002;
pub const IPMSG_FILE_RETPARENT: u32 = 0x00000003;
pub const IPMSG_FILE_SYMLINK: u32 = 0x00000004;
pub const IPMSG_FILE_CDEV: u32 = 0x00000005;
pub const IPMSG_FILE_BDEV: u32 = 0x00000006;
pub const IPMSG_FILE_FIFO: u32 = 0x00000007;
pub const IPMSG_FILE_RESFORK: u32 = 0x00000010; // Mac resource fork

// ─── File attribute options ──────────────────────────────────

pub const IPMSG_FILE_RONLYOPT: u32 = 0x00000100;
pub const IPMSG_FILE_HIDDENOPT: u32 = 0x00001000;
pub const IPMSG_FILE_EXHIDDENOPT: u32 = 0x00002000;
pub const IPMSG_FILE_ARCHIVEOPT: u32 = 0x00004000;
pub const IPMSG_FILE_SYSTEMOPT: u32 = 0x00008000;

// ─── Separators (exact byte values from original) ────────────

/// File list separator (0x07, ASCII BEL)
pub const FILELIST_SEPARATOR: u8 = 0x07;
/// Host list dummy placeholder for ':' in names (0x08, ASCII BS)
pub const HOSTLIST_DUMMY: u8 = 0x08;
/// Host list entry separator (0x3A, ':')
pub const HLIST_ENTRY_SEPARATOR: u8 = 0x3A;
/// Null terminator
pub const MSG_NULL: u8 = 0x00;

// ─── Folder transfer TCP protocol markers (feiq++ custom) ─────

/// Sent by receiver over TCP to request the folder manifest
pub const FOLDER_MANIFEST_REQUEST: &[u8] = b"FOLDER_MANIFEST_REQUEST\n";
/// Sent by sender after all files have been transferred successfully
pub const FOLDER_TRANSFER_COMPLETE: &[u8] = b"FOLDER_TRANSFER_COMPLETE\n";
/// Sent by either side to gracefully cancel the folder transfer
pub const FOLDER_TRANSFER_CANCEL: &[u8] = b"FOLDER_TRANSFER_CANCEL\n";
/// Sent by receiver to signal it's ready for the next file (acknowledges previous)
pub const FOLDER_FILE_ACK: &[u8] = b"FOLDER_FILE_ACK\n";

// ─── Helper functions ────────────────────────────────────────

/// Check if command matches (compare low 8 bits)
#[inline]
pub fn is_cmd_set(cmd: u32, test: u32) -> bool {
    (cmd & 0xFF) == test
}

/// Check if option flag is set
#[inline]
pub fn is_opt_set(cmd: u32, opt: u32) -> bool {
    (cmd & opt) == opt
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    // ─── File/folder constant uniqueness ─────────────────────

    #[test]
    fn file_type_values_are_unique() {
        let ft: [u32; 8] = [
            IPMSG_FILE_REGULAR,
            IPMSG_FILE_DIR,
            IPMSG_FILE_RETPARENT,
            IPMSG_FILE_SYMLINK,
            IPMSG_FILE_CDEV,
            IPMSG_FILE_BDEV,
            IPMSG_FILE_FIFO,
            IPMSG_FILE_RESFORK,
        ];
        let mut seen = HashSet::new();
        for v in ft {
            assert!(seen.insert(v), "duplicate file type value: 0x{:08X}", v);
        }
    }

    #[test]
    fn file_attr_values_are_unique() {
        let fa: [u32; 5] = [
            IPMSG_FILE_RONLYOPT,
            IPMSG_FILE_HIDDENOPT,
            IPMSG_FILE_EXHIDDENOPT,
            IPMSG_FILE_ARCHIVEOPT,
            IPMSG_FILE_SYSTEMOPT,
        ];
        let mut seen = HashSet::new();
        for v in fa {
            assert!(seen.insert(v), "duplicate file attr value: 0x{:08X}", v);
        }
    }

    #[test]
    fn file_types_and_attrs_do_not_overlap() {
        let ft: [u32; 8] = [
            IPMSG_FILE_REGULAR,
            IPMSG_FILE_DIR,
            IPMSG_FILE_RETPARENT,
            IPMSG_FILE_SYMLINK,
            IPMSG_FILE_CDEV,
            IPMSG_FILE_BDEV,
            IPMSG_FILE_FIFO,
            IPMSG_FILE_RESFORK,
        ];
        let fa: [u32; 5] = [
            IPMSG_FILE_RONLYOPT,
            IPMSG_FILE_HIDDENOPT,
            IPMSG_FILE_EXHIDDENOPT,
            IPMSG_FILE_ARCHIVEOPT,
            IPMSG_FILE_SYSTEMOPT,
        ];
        for t in &ft {
            for a in &fa {
                assert_ne!(
                    t, a,
                    "file type 0x{:08X} collides with file attr 0x{:08X}",
                    t, a
                );
            }
        }
    }

    // ─── Marker byte sequence distinctness ──────────────────

    #[test]
    fn folder_marker_bytes_are_distinct() {
        let markers: [&[u8]; 4] = [
            FOLDER_MANIFEST_REQUEST,
            FOLDER_TRANSFER_COMPLETE,
            FOLDER_TRANSFER_CANCEL,
            FOLDER_FILE_ACK,
        ];
        // No marker should be a prefix of another (wire protocol safety)
        for (i, a) in markers.iter().enumerate() {
            for (j, b) in markers.iter().enumerate() {
                if i == j {
                    continue;
                }
                let a_is_prefix = a.len() <= b.len() && &b[..a.len()] == *a;
                assert!(
                    !a_is_prefix,
                    "{:?} is a prefix of {:?}",
                    String::from_utf8_lossy(a),
                    String::from_utf8_lossy(b)
                );
            }
        }
        // All markers are non-empty and end with newline
        for m in &markers {
            assert!(!m.is_empty(), "marker is empty");
            assert_eq!(m[m.len() - 1], b'\n', "marker {:?} must end with newline", String::from_utf8_lossy(m));
        }
    }

    #[test]
    fn folder_marker_lengths_are_unique() {
        let markers: [&[u8]; 4] = [
            FOLDER_MANIFEST_REQUEST,
            FOLDER_TRANSFER_COMPLETE,
            FOLDER_TRANSFER_CANCEL,
            FOLDER_FILE_ACK,
        ];
        let mut seen = HashSet::new();
        for m in &markers {
            assert!(seen.insert(m.len()), "duplicate marker length {}", m.len());
        }
    }

    // ─── Separator byte uniqueness ──────────────────────────

    #[test]
    fn separator_bytes_are_distinct() {
        let seps: [u8; 4] = [
            FILELIST_SEPARATOR,
            HOSTLIST_DUMMY,
            HLIST_ENTRY_SEPARATOR,
            MSG_NULL,
        ];
        let mut seen = HashSet::new();
        for s in seps {
            assert!(seen.insert(s), "duplicate separator byte 0x{:02X}", s);
        }
    }
}

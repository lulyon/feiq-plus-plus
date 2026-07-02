# Planned but Unimplemented Features

## Overview

| Phase | Status | Key Deliverables |
|-------|--------|-----------------|
| Phase 1 — MVP | **100%** | Basic messaging |
| Phase 2 — File Transfer + Emoji | **100%** | File transfer, emoji, knock |
| Phase 3 — Chat History + Screenshot + User Mgmt | **100%** | History, search, screenshot annotation, contact groups, alias |
| Phase 4 — Group Chat + Offline Messages | **85%** | Group chat, offline messages, blacklist; folder transfer **deferred** |
| Phase 4.5 — Relay Server | **100%** | WebSocket relay for cross-network chat |
| Phase 5 — Encryption + File Share + Polish | **100%** | E2E encryption, sealed messages, themes, import/export |
| Phase 6 — IPMSG v9 Legacy Encryption | **0% (deferred)** | RSA/RC2/Blowfish interop with legacy clients |

---

## Deferred: Folder Transfer

- **Status**: Removed from codebase (commit f6c7616).
- **Reason**: Custom feiq++ extension. Original feiq doesn't support it. Added complexity without practical benefit.
- **Protocol**: `IPMSG_GETDIRFILES` (0x62) is defined but rejected by the engine. The engine filters out directory entries in message contents.

---

## Deferred: IPMSG v9 Legacy Encryption (Phase 6)

- **Status**: Not started.
- **Scope**: RSA key exchange, RC2/Blowfish/AES symmetric encryption, MD5/SHA1 digital signatures.
- **Reason**: Required for encrypted interop with legacy feiq clients. feiq++ uses modern `ring`-based ECDH + AES-256-GCM for feiq++ ↔ feiq++ encryption.
- **Dependencies not yet added**: `rsa = "0.9"`, `blowfish = "0.10"`, `sha1 = "0.11"`, `md5 = "0.8"`.

---

## Unimplemented Features

### File Download via Relay

- **Priority**: Medium
- **Status**: Not implemented.
- **Details**: Peers connected through the relay server cannot download files. A guard in `commands.rs` returns: `"File download not supported for relay peers. Use direct LAN connection."`
- **Reason**: File transfer uses direct TCP connections; relay uses WebSocket. No file tunneling through relay exists.

### Typing Indicator (`IPMSG_INPUTING` / `IPMSG_INPUT_END`)

- **Priority**: Low (P3)
- **Status**: Not implemented.
- **Details**: `IPMSG_INPUTING` (0x79) and `IPMSG_INPUT_END` (0x7A) constants are defined in `protocol/constants.rs`, but no parser handler or engine logic exists.
- **Original feiq**: Supported.

### Transfer Speed Limiting

- **Priority**: Low (P4)
- **Status**: Not implemented.
- **Details**: No token bucket or rate limiting for file transfers. `send_file` / `recv_file` transfer at full available bandwidth.

### File Share (Password-Protected)

- **Priority**: Low (P5)
- **Status**: **Partial** — backend skeleton exists, no frontend UI.
- **Details**:
  - `AppConfig` has `shared_dir` and `shared_dir_password` fields (`settings.rs:66-71`).
  - `handle_file_share_request` method exists in `engine.rs`.
  - `list_directory` function in `tcp.rs` walks a directory for sharing.
  - No frontend UI in SettingsDialog to configure shared directory or password.

### Group Features (P5)

| Feature | Status |
|---------|--------|
| In-group file sharing | Not implemented |
| Group announcements | Not implemented |

### User Management (P5)

| Feature | Status |
|---------|--------|
| Group-level permission control | Not implemented |
| Stealth mode (global + per-group) | Not implemented |
| Personal avatar / profile picture | Data model field exists; no upload/selection UI (P3) |

### Personalization (P5)

| Feature | Status |
|---------|--------|
| Full theme skinning (background images, custom color schemes) | Not implemented. Light/dark/auto theme exists. |
| UI font customization | Not implemented |
| Standalone doodle / drawing tool | Not implemented. Screenshot annotation exists. |

### Pinyin-Based Contact Search

- **Priority**: Low
- **Status**: Not implemented.
- **Details**: `ContactBook::search` doc says "Search contacts by name, IP, host, or pinyin" but only matches against display name, IP, and host. No pinyin conversion logic exists.

---

## Permanent Limitations

### Inline Image Protocol

- **Details**: `IPMSG_SENDIMAGE` (0xC0) provides only an 8-char ID. The actual image data channel was never reverse-engineered. The parser extracts the ID as `Content::Image`, but the engine discards it and replies: `"feiq++ does not support inline images. Please send as file."`
- **Workaround**: Send images via file transfer (`build_file_message` + TCP file data).

### GIF Animated Emoji

- **Details**: 96 QQ-style emoji codes are implemented, but using **static PNG** images. GIF was intentionally avoided for performance reasons.

### Out-of-Scope Features

These are explicitly considered beyond the scope of an IM application:

| Feature | Note |
|---------|------|
| Voice chat | Beyond IM scope |
| Remote desktop | Beyond IM scope |
| Schedule / calendar | Beyond IM scope |
| FeiQ Space blog | Obsolete, omitted |
| FeiQ App Manager | Obsolete, omitted |
| FeiQ Bot (CLI automation) | Obsolete, omitted |

---

## Unhandled IPMSG Commands

15+ IPMSG protocol commands are defined in `protocol/constants.rs` but have no parser handler or engine logic. They are non-essential for core chat functionality:

| Command | Constant | Value | Purpose |
|---------|----------|-------|---------|
| BR_ABSENCE | `IPMSG_BR_ABSENCE` | 0x04 | Absence mode / status change |
| BR_ISGETLIST | `IPMSG_BR_ISGETLIST` | 0x10 | Request member list |
| OKGETLIST | `IPMSG_OKGETLIST` | 0x11 | Acknowledge list |
| GETLIST | `IPMSG_GETLIST` | 0x12 | Get member list |
| ANSLIST | `IPMSG_ANSLIST` | 0x13 | Return member list |
| BR_ISGETLIST2 | `IPMSG_BR_ISGETLIST2` | 0x18 | Request list v2 |
| READMSG | `IPMSG_READMSG` | 0x30 | Message read (sealed) |
| DELMSG | `IPMSG_DELMSG` | 0x31 | Delete message |
| ANSREADMSG | `IPMSG_ANSREADMSG` | 0x32 | Read receipt (v8+) |
| GETINFO | `IPMSG_GETINFO` | 0x40 | Request protocol version |
| SENDINFO | `IPMSG_SENDINFO` | 0x41 | Send protocol version |
| GETABSENCEINFO | `IPMSG_GETABSENCEINFO` | 0x50 | Ask if away |
| SENDABSENCEINFO | `IPMSG_SENDABSENCEINFO` | 0x51 | Reply away status |
| GETPUBKEY | `IPMSG_GETPUBKEY` | 0x72 | Request RSA public key |
| ANSPUBKEY | `IPMSG_ANSPUBKEY` | 0x73 | Reply RSA public key |
| INPUTING | `IPMSG_INPUTING` | 0x79 | Typing indicator |
| INPUT_END | `IPMSG_INPUT_END` | 0x7A | Typing ended |
| SENDIMAGE_DATA | `IPMSG_SENDIMAGE_DATA` | 0xC1 | Image data (custom) |

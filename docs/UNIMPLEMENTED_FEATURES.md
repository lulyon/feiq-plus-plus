# Planned but Unimplemented Features

## Overview

| Phase | Status | Key Deliverables |
|-------|--------|-----------------|
| Phase 1 — MVP | **100%** | Basic messaging |
| Phase 2 — File Transfer + Emoji | **100%** | File transfer, emoji, knock |
| Phase 3 — Chat History + User Mgmt | **100%** | History, search, contact groups, alias |
| Phase 4 — Group Chat + Offline Messages | **85%** | Group chat, offline messages, blacklist; folder transfer **deferred** |
| Phase 4.5 — Relay Server | **100%** | WebSocket relay for cross-network chat |
| Phase 5 — Encryption + File Share + Polish | **100%** | E2E encryption, sealed messages, themes, import/export |
| Phase 6 — Enhancements | **100%** | Typing indicators, relay file DL, stealth mode, avatar, speed limiting, pinyin search, file share, group features, theme, font, doodle |
| Phase 7 — IPMSG v9 Legacy Encryption | **0% (deferred)** | RSA/RC2/Blowfish interop with legacy clients |

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

### File Download via Relay ✅

- **Priority**: Medium
- **Status**: **Implemented**.
- **Details**: Binary WebSocket tunnel relays file chunks between peers. Relay client sends/receives `FileStart`/`FileEnd` JSON messages + binary frames with `[8 bytes file_id BE][chunk data]` format. Engine uses push model for relay file sends. Relay guard in `commands.rs` removed.
- **Files**: `relay/server.rs`, `relay/client.rs`, `engine.rs`, `commands.rs`, `network/mod.rs`

### Typing Indicator (`IPMSG_INPUTING` / `IPMSG_INPUT_END`) ✅

- **Priority**: Medium
- **Status**: **Implemented**.
- **Details**: Full-stack: parser handlers (`RecvInputing`/`RecvInputEnd`), `Content::Typing` variant, `FrontendEvent::TypingIndicator`, IPC `send_typing` command, frontend `InputArea.tsx` debounce detection, `ChatPanel.tsx` animated display, `typingStore.ts` with 5s auto-clear.

### Transfer Speed Limiting ✅

- **Priority**: Medium
- **Status**: **Implemented**.
- **Details**: Config fields `upload_speed_limit_kbps`/`download_speed_limit_kbps` (0=unlimited). Sleep-based pacing in `send_file`/`recv_file` after each chunk. Engine passes config values to TCP transfer. 2 tests verify throttling.

### File Share (Password-Protected) ✅

- **Priority**: Medium
- **Status**: **Implemented**.
- **Details**: Standalone `check_file_share_request` with password validation via `IPMSG_PASSWORDOPT`. `browse_shared_folder` IPC command. `RemoteFileBrowser.tsx` frontend modal. Engine UDP event loop wired to call file share handler. 4 password tests.

### Group Features ✅

| Feature | Status |
|---------|--------|
| In-group file sharing | **Implemented** — `send_file_to_group()` in engine, `send_group_file` IPC, file button in `GroupChatPanel` |
| Group announcements | **Implemented** — `group_announcements` table, `save_announcement`/`get_announcements` in HistoryDb, `send_announcement_to_group` P2P dispatch, IPC commands |

### User Management ✅

| Feature | Status |
|---------|--------|
| Group-level permission control | **Implemented** — `owner_ip` + `settings` columns in `groups_info`, `save_group_with_owner`/`delete_group`/`update_group_settings`, `delete_group_cmd` IPC |
| Stealth mode (global) | **Implemented** — `AppConfig.stealth_mode`, skips BR_ENTRY broadcast + ANSENTRY reply, `set_stealth_mode` IPC |
| Personal avatar / profile picture | **Implemented** — `Fellow.avatar_hash`, `contact_meta.avatar_hash`, `AppConfig.avatar_path`, `IPMSG_GETAVATAR`/`IPMSG_SENDAVATAR` protocol handlers, `set_avatar`/`get_avatar` IPC, SHA256 hash exchange |

### Personalization ✅

| Feature | Status |
|---------|--------|
| Full theme skinning | **Implemented** — `CustomTheme` struct (bg, surface, primary, text, bubble_sent, bubble_recv), runtime CSS variable injection in `App.tsx` |
| UI font customization | **Implemented** — `font_family`/`font_size` config fields, CSS `--font-family`/`--font-size` variables, `App.tsx` injection |
| Standalone doodle / drawing tool | **Implemented** — `DoodleDialog.tsx`, HTML5 Canvas (pen, eraser, 10 colors, line width, undo), button in `InputArea.tsx` |

### Pinyin-Based Contact Search ✅

- **Priority**: High
- **Status**: **Implemented**.
- **Details**: `pinyin` crate in `feiq-core/Cargo.toml`. `ContactBook::search` enhanced with first-letter and full-pinyin matching. Frontend `Sidebar.tsx` search includes `pc_name` and `host` fields.

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

| Command | Constant | Value | Purpose | Status |
|---------|----------|-------|---------|--------|
| BR_ABSENCE | `IPMSG_BR_ABSENCE` | 0x04 | Absence mode / status change | ✅ Implemented |
| BR_ISGETLIST | `IPMSG_BR_ISGETLIST` | 0x10 | Request member list | Deferred |
| OKGETLIST | `IPMSG_OKGETLIST` | 0x11 | Acknowledge list | Deferred |
| GETLIST | `IPMSG_GETLIST` | 0x12 | Get member list | Deferred |
| ANSLIST | `IPMSG_ANSLIST` | 0x13 | Return member list | Deferred |
| BR_ISGETLIST2 | `IPMSG_BR_ISGETLIST2` | 0x18 | Request list v2 | Deferred |
| READMSG | `IPMSG_READMSG` | 0x30 | Message read (sealed) | ✅ Implemented |
| DELMSG | `IPMSG_DELMSG` | 0x31 | Delete message | Deferred (privacy) |
| ANSREADMSG | `IPMSG_ANSREADMSG` | 0x32 | Read receipt (v8+) | ✅ Implemented |
| GETINFO | `IPMSG_GETINFO` | 0x40 | Request protocol version | Low priority |
| SENDINFO | `IPMSG_SENDINFO` | 0x41 | Send protocol version | Low priority |
| GETABSENCEINFO | `IPMSG_GETABSENCEINFO` | 0x50 | Ask if away | ✅ Impl. (stealth) |
| SENDABSENCEINFO | `IPMSG_SENDABSENCEINFO` | 0x51 | Reply away status | ✅ Impl. (stealth) |
| GETPUBKEY | `IPMSG_GETPUBKEY` | 0x72 | Request RSA public key | Deferred (P7) |
| ANSPUBKEY | `IPMSG_ANSPUBKEY` | 0x73 | Reply RSA public key | Deferred (P7) |
| INPUTING | `IPMSG_INPUTING` | 0x79 | Typing indicator | ✅ Implemented |
| INPUT_END | `IPMSG_INPUT_END` | 0x7A | Typing ended | ✅ Implemented |
| GETAVATAR | `IPMSG_GETAVATAR` | 0x75 | Request avatar (custom) | ✅ Implemented |
| SENDAVATAR | `IPMSG_SENDAVATAR` | 0x76 | Send avatar (custom) | ✅ Implemented |
| SENDIMAGE_DATA | `IPMSG_SENDIMAGE_DATA` | 0xC1 | Image data (custom) | N/A |

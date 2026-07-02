# CLAUDE.md — feiq++ Project Context

## Project Identity
- **Name**: feiq-plus-plus
- **Purpose**: Modern cross-platform LAN + Relay chat app implementing IP Messenger protocol
- **Origin**: Rewrite of feiq (Qt5/C++ macOS-only) using Rust + Tauri + React
- **Status**: Phase 1-5 complete, all planned features implemented. 200 Rust + 18 TS tests pass, 0 compile errors
- **Version**: 0.1.4

## Build & Test

```bash
cargo check --workspace     # fast compile check
cargo test --workspace       # run all 200 Rust tests + 18 TS tests (218 total)
cargo build --workspace      # full build
cargo tauri dev              # dev mode with hot-reload frontend

# Multi-instance testing (2 peers on same machine):
npm --prefix packages/feiq-gui run build   # build frontend once
./scripts/dev-multi.sh                     # launches Alice:2425 + Bob:2426
# Or manual:
FEIQ_NAME=Alice cargo run --package feiq-app
FEIQ_NAME=Bob FEIQ_PORT=2426 cargo run --package feiq-app

# Relay server (for cross-network communication):
cargo run --package feiq-relay -- --host 0.0.0.0 --port 9001
```

## Architecture (Hybrid LAN + Relay)

```
React Frontend (View)
  ↕ Tauri IPC (commands + events)
Rust Engine (Controller)
  ↕ mpsc channels              ↕ WebSocket
Network Layer                   Relay Client
  ↕ UDP (port 2425)             (relay.rs)
  ↕ TCP (file transfer)
  ↕ IPMSG Protocol
LAN Peers                feiq-relay Server
                              ↕ WebSocket
                          Remote Peers
```

## Workspace Crates

| Crate | Purpose |
|-------|---------|
| `feiq-core` | Protocol engine, network (UDP/TCP/Relay), encryption, storage |
| `feiq-app` | Tauri desktop shell, IPC commands, tray, event forwarding |
| `feiq-relay` | Standalone WebSocket relay server for cross-network chat |

## Key Files

### Protocol Layer (`packages/feiq-core/src/protocol/`)
| File | Purpose |
|------|---------|
| `constants.rs` | All IPMSG v9 + FeiQ extension constants |
| `types.rs` | Fellow, Content, Post, FileContent, FileTask, ContactSource |
| `encoding.rs` | GBK ↔ UTF-8 via encoding_rs |
| `serializer.rs` | pack_message, parse_raw, parse_version_info |
| `parser.rs` | 19-handler chain-of-responsibility protocol parser |
| `emoji.rs` | 96 QQ-style emoji code ↔ name mapping |

### Network Layer (`packages/feiq-core/src/network/`)
| File | Purpose |
|------|---------|
| `udp.rs` | tokio UDP socket, broadcast, MAC detection |
| `tcp.rs` | 64KB chunk file transfer, send_file/recv_file with rate limiting |
| `relay.rs` | WebSocket relay client transport with binary file transfer support |
| `manager.rs` | Coordinates UDP+TCP+Relay, parse→dispatch cycle |
| `crypto.rs` | ECDH (x25519) + AES-256-GCM, only feiq++ ↔ feiq++ |

### Engine Layer (`packages/feiq-core/src/engine/`)
| File | Purpose |
|------|---------|
| `engine.rs` | Main controller, hybrid LAN+Relay, protocol message builders, event dispatch |
| `events.rs` | FrontendEvent enum (ContactUpdate, NewMessage, FileProgress, etc.) |
| `tasks.rs` | FileTaskHandle with progress throttling (1%/100KB) |

### Model & Storage (`packages/feiq-core/src/`)
| File | Purpose |
|------|---------|
| `model/contacts.rs` | Thread-safe ContactBook (IP-indexed, MAC-dedup, Relay peers) |
| `storage/settings.rs` | INI config (~/.feiq_setting.ini), ConnectionMode (lan/relay/hybrid) |
| `storage/history.rs` | SQLite chat history, pending messages, groups |

### Tauri Bridge (`packages/feiq-app/src/`)
| File | Purpose |
|------|---------|
| `commands.rs` | 36 IPC commands (start_engine, stop_engine, get_contacts, search_contacts, add_contact, get_settings, update_settings, get_chat_history, search_chat_history, get_emoji_list, send_knock, send_text, send_read_receipt, send_typing, set_alias, set_contact_group, create_group, delete_group_cmd, get_groups, send_group_text, send_group_file, send_group_announcement, get_group_announcements, export_history, import_history, add_to_blacklist, remove_from_blacklist, get_blacklist, reset_unread_count, browse_shared_folder, set_avatar, get_avatar, set_stealth_mode, download_file, cancel_file_task, send_file) |
| `state.rs` | AppState (Engine + Config + event channels) |
| `events.rs` | Forwards FrontendEvent → Tauri window events |
| `tray.rs` | System tray icon + context menu |

### Relay Server (`packages/feiq-relay/src/`)
| File | Purpose |
|------|---------|
| `server.rs` | Core RelayServer — WebSocket message router, rooms, offline queue, binary file chunk relay |
| `main.rs` | CLI entry (clap args: --host, --port) |
| `lib.rs` | Re-exports RelayServer |

### React Frontend (`packages/feiq-gui/src/`)
| File | Purpose |
|------|---------|
| `App.tsx` | Root: Tauri event listeners, engine auto-start, theme/font injection |
| `components/Sidebar.tsx` | Contact list, search, online count, unread badges, group tree |
| `components/ChatPanel.tsx` | Chat header + infinite-scroll message list + input area + Browse Files button |
| `components/MessageBubble.tsx` | Text/knock/file bubbles + emoji inline rendering |
| `components/InputArea.tsx` | Text input + emoji picker toggle + doodle button + send button |
| `components/EmojiPicker.tsx` | 16×6 emoji grid with hover preview |
| `components/SettingsDialog.tsx` | Config editor (name, host, connection mode, relay URL, IP ranges, send_by_enter, theme) |
| `components/CreateGroupDialog.tsx` | Group creation modal (name, member selection) |
| `components/FileTransferPanel.tsx` | File transfer list with progress bars, send/recv status, cancel/resume |
| `components/DoodleDialog.tsx` | HTML5 Canvas freehand drawing tool (pen, eraser, colors, undo) |
| `components/RemoteFileBrowser.tsx` | Browse remote shared directories with password support |
| `stores/contactStore.ts` | Zustand: contacts list, upsert, select, alias editing |
| `stores/messageStore.ts` | Zustand: messages by IP, unread counts, history search |
| `stores/fileTransferStore.ts` | Zustand: file transfer queue, progress, status per task |
| `stores/groupStore.ts` | Zustand: groups, members, invitations |
| `stores/typingStore.ts` | Zustand: typing indicator state with auto-clear timeout |

## Protocol Details

- **Port**: 2425 UDP (messaging) + TCP (file transfer)
- **Relay port**: configurable (default 9001) WebSocket
- **Wire format**: `version:packetNo:name:host:cmdId:extra\0`
- **Encoding**: GBK for legacy compatibility, UTF-8 internally
- **Self-filter**: Drop packets where MAC AND name both match self
- **Version string**: `feiq_plus_plus#128#MAC#0#0#0#1#9`
- **ContactSource**: `Lan` (UDP-discovered) or `Relay` (via relay server)

## Connection Modes

| Mode | Description |
|------|-------------|
| `LanOnly` | Traditional UDP broadcast on port 2425 (default) |
| `RelayOnly` | WebSocket to relay server, no LAN traffic |
| `Hybrid` | Both LAN and relay simultaneously, deduplicates peers |

## Key Design Decisions

1. **ring not RSA**: Use modern ECDH+AES-GCM, skip IPMSG v9 legacy RSA/Blowfish
2. **Images via files**: IPMSG_SENDIMAGE only provides 8-char ID, data channel uncracked → use file transfer fallback
3. **File chunks**: 64KB for feiq++ ↔ feiq++, compatible with legacy 2KB
4. **Group chat**: P2P dispatch (send to each member individually), no server
5. **Relay server**: Self-built Rust WebSocket server — custom JSON protocol (10 msg types including FileStart/FileEnd/FileChunk), room-based routing, binary file chunk relay, 24h offline queue, E2E transparent
6. **dingo**: Use `LessSafeKey` not `SealingKey` — `SealingKey::new` is on `BoundKey` trait, not inherent; `UnboundKey` not Clone
7. **File transfer engine**: FileTaskHandle state machine with progress throttling (1%/100KB), implements IPMSG GETFILEDATA protocol for pull-based transfers, supports cancel/resume
8. **Theme system**: CSS variables via Tailwind v4 `@theme` directive — light/dark/auto/custom with CSS `prefers-color-scheme` detection and runtime variable injection, persisted in settings
9. **File share**: IPMSG GETDIRFILES with password-protected shared directory browsing via `IPMSG_PASSWORDOPT` flag
10. **Stealth mode**: Global invisibility — no BR_ENTRY broadcast, no ANSENTRY reply
11. **Avatar**: SHA256-hashed avatar exchange via custom `IPMSG_GETAVATAR`/`IPMSG_SENDAVATAR` protocol extension
12. **Speed limiting**: Sleep-based rate limiting in file transfer (send_file/recv_file), configurable per direction
13. **Typing indicator**: `IPMSG_INPUTING`/`IPMSG_INPUT_END` with debounced frontend detection and 5s auto-clear

## Known Limitations
- Image protocol data channel not reverse-engineered
- Voice chat not implemented (beyond IM scope)
- Remote desktop not implemented (beyond IM scope)
- Schedule/calendar not implemented (beyond IM scope)
- IPMSG v9 legacy encryption (RSA/RC2/Blowfish) deferred
- Folder transfer removed from codebase
- Screenshot annotation removed from codebase
- Full theme skinning UI (color pickers in Settings) pending
- Group permission enforcement is advisory (P2P model)
- Doodle tool send-via-file-transfer pending full plumbing

## Dependencies
- **Rust**: tokio(full), tokio-tungstenite, encoding_rs, rusqlite(bundled), ring 0.17, serde, chrono, mac_address, base64, sha2, hex, pinyin, futures-util
- **Relay**: clap 4 (derive), uuid 1
- **Tauri**: 2.11.3 + notification/dialog/global-shortcut/fs plugins
- **Frontend**: react 18, zustand 4, tailwindcss 3, lucide-react, @tauri-apps/api 2.x, @tauri-apps/plugin-dialog, @tauri-apps/plugin-notification, @tauri-apps/plugin-fs

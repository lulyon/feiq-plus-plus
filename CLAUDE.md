# CLAUDE.md — feiq++ Project Context

## Project Identity
- **Name**: feiq-plus-plus
- **Purpose**: Modern cross-platform LAN chat app implementing IP Messenger protocol
- **Origin**: Rewrite of feiq (Qt5/C++ macOS-only) using Rust + Tauri + React
- **Status**: Phase 1-5 complete, 27 tests pass, 0 compile errors

## Build & Test

```bash
cargo check --workspace     # fast compile check
cargo test --workspace       # run all 27 tests
cargo build --workspace      # full build
cargo tauri dev              # dev mode with hot-reload frontend
```

## Architecture (MVC-like)

```
React Frontend (View)
  ↕ Tauri IPC (commands + events)
Rust Engine (Controller)
  ↕ mpsc channels
Network Layer (UDP port 2425 + TCP file transfer)
  ↕ IPMSG Protocol
LAN Peers
```

## Key Files

### Protocol Layer (`packages/feiq-core/src/protocol/`)
| File | Purpose |
|------|---------|
| `constants.rs` | All IPMSG v9 + FeiQ extension constants |
| `types.rs` | Fellow, Content, Post, FileContent, FileTask |
| `encoding.rs` | GBK ↔ UTF-8 via encoding_rs |
| `serializer.rs` | pack_message, parse_raw, parse_version_info |
| `parser.rs` | 12-handler chain-of-responsibility protocol parser |
| `emoji.rs` | 96 QQ-style emoji code ↔ name mapping |

### Network Layer (`packages/feiq-core/src/network/`)
| File | Purpose |
|------|---------|
| `udp.rs` | tokio UDP socket, broadcast, MAC detection |
| `tcp.rs` | 64KB chunk file transfer, send_file/recv_file |
| `manager.rs` | Coordinates UDP+TCP, parse→dispatch cycle |
| `crypto.rs` | ECDH (x25519) + AES-256-GCM, only feiq++ ↔ feiq++ |

### Engine Layer (`packages/feiq-core/src/engine/`)
| File | Purpose |
|------|---------|
| `engine.rs` | Main controller, protocol message builders, event dispatch |
| `events.rs` | FrontendEvent enum (ContactUpdate, NewMessage, FileProgress, etc.) |
| `tasks.rs` | FileTaskHandle with progress throttling (1%/100KB) |

### Model & Storage
| File | Purpose |
|------|---------|
| `model/contacts.rs` | Thread-safe ContactBook (IP-indexed, MAC-dedup) |
| `storage/settings.rs` | INI config (~/.feiq_setting.ini) load/save |
| `storage/history.rs` | SQLite chat history, pending messages, groups |

### Tauri Bridge (`packages/feiq-app/src/`)
| File | Purpose |
|------|---------|
| `commands.rs` | 9 IPC commands (start_engine, get_contacts, send_knock, etc.) |
| `state.rs` | AppState (Engine + Config + event channels) |
| `events.rs` | Forwards FrontendEvent → Tauri window events |
| `tray.rs` | System tray icon + context menu |

### React Frontend (`packages/feiq-gui/src/`)
| File | Purpose |
|------|---------|
| `App.tsx` | Root: Tauri event listeners, engine auto-start |
| `components/Sidebar.tsx` | Contact list, search, online count, unread badges |
| `components/ChatPanel.tsx` | Chat header + message list + input area |
| `components/MessageBubble.tsx` | Text/knock/file bubbles + emoji inline rendering |
| `components/InputArea.tsx` | Text input + emoji picker toggle + send button |
| `components/EmojiPicker.tsx` | 16×6 emoji grid with hover preview |
| `components/SettingsDialog.tsx` | Config editor (name, host, IP ranges, send_by_enter) |
| `stores/contactStore.ts` | Zustand: contacts list, upsert, select |
| `stores/messageStore.ts` | Zustand: messages by IP, unread counts |

## Protocol Details

- **Port**: 2425 UDP (messaging) + TCP (file transfer)
- **Wire format**: `version:packetNo:name:host:cmdId:extra\0`
- **Encoding**: GBK for legacy compatibility, UTF-8 internally
- **Self-filter**: Drop packets where MAC AND name both match self
- **Version string**: `feiq_plus_plus#128#MAC#0#0#0#1#9`

## Key Design Decisions

1. **ring not RSA**: Use modern ECDH+AES-GCM, skip IPMSG v9 legacy RSA/Blowfish
2. **Images via files**: IPMSG_SENDIMAGE only provides 8-char ID, data channel uncracked → use file transfer fallback
3. **File chunks**: 64KB for feiq++ ↔ feiq++, compatible with legacy 2KB
4. **Group chat**: P2P dispatch (send to each member individually), no server
5. **dingo**: Use `LessSafeKey` not `SealingKey` — `SealingKey::new` is on `BoundKey` trait, not inherent; `UnboundKey` not Clone

## Known Limitations
- Image protocol data channel not reverse-engineered
- Voice chat not implemented (beyond IM scope)
- Remote desktop not implemented (beyond IM scope)
- Schedule/calendar not implemented (beyond IM scope)
- IPMSG v9 legacy encryption (RSA/RC2/Blowfish) deferred to P6

## Dependencies
- **Rust**: tokio(full), encoding_rs, rusqlite(bundled), ring 0.17, serde, chrono, mac_address
- **Tauri**: 2.11.3 + notification/dialog/global-shortcut/fs plugins
- **Frontend**: react 18, zustand 4, tailwindcss 3, lucide-react, @tauri-apps/api 2.x

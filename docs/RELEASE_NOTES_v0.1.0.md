# feiq++ v0.1.4 — Release Notes

The fourth release of **feiq++**, a modern cross-platform LAN + Relay instant messaging app.  
Implements the full IP Messenger (飞鸽传书) Draft-9 protocol with FeiQ (飞秋) extensions.  
Built with **Rust + Tauri 2 + React**, delivering native performance with a modern UI.

---

## Highlights

- **Hybrid LAN + Relay architecture** — auto-discovers LAN peers via UDP broadcast (port 2425) AND connects to remote peers via self-built WebSocket relay server
- **Full IPMSG v9 protocol** — complete command/option set including encryption flags
- **FeiQ protocol extensions** — 96 QQ-style emoji, window shake, custom broadcast, file transfer
- **End-to-end encryption** — ECDH (x25519) + AES-256-GCM between feiq++ peers (random nonce prefix prevents reuse)
- **Cross-platform** — macOS (Intel + Apple Silicon), Windows, Linux
- **100-agent security audit** — 17 issues found and fixed, 0 remaining. Includes critical AES-GCM nonce reuse fix
- **77 Rust + 18 TypeScript = 95 tests** passing, zero compile errors

---

## Features

### Messaging
- Text messages with instant send/receive
- 96 built-in QQ-style emoji (inline rendering in chat)
- Window shake / knock
- Configurable Enter / Ctrl+Enter to send
- Offline message queue (auto-delivered when peer comes online)

### File Transfer
- Single file transfer with real-time progress bar
- 100GB+ large file support with size validation
- Drag-and-drop file sending
- Resume (offset-based continuation)
- Cancel support
- Chunk size: 64KB (feiq++ ↔ feiq++) / 2KB (legacy compatible)

### Relay Server (NEW in v0.1.4)
- Standalone Rust WebSocket relay server (`feiq-relay`)
- 7 message types (Join/Leave/Send/Broadcast/Ping + Joined/PeerOnline/PeerOffline/Message/Broadcast/OfflineMsgs/Pong/Error)
- Three connection modes: LAN Only / Relay Only / Hybrid
- Cross-transport deduplication (MAC + name)
- Offline message queue with 24h TTL and stable peer_key routing
- 200 messages per peer DoS protection

### Contacts & Groups
- Auto-discovery of LAN + Relay users
- Contact grouping with tree view
- Custom aliases and signatures
- Name/IP search
- Custom broadcast IP ranges (for cross-subnet discovery)
- P2P group chat (no server needed)
- Blacklist filtering

### Security
- ECDH (x25519) key exchange + AES-256-GCM encryption
- Random nonce prefix prevents reuse across sessions
- Only activated between feiq++ peers (detected via version string)
- Sealed messages (self-destruct / 阅后即焚)
- Legacy clients communicate in plaintext

### Screenshot & Annotation
- Cross-platform screenshot capture
- Canvas-based annotation: freehand drawing, rectangles, arrows, text
- Color selection and undo support
- Annotation exported as image via file transfer

### Chat History
- SQLite persistent storage with full-text search
- Infinite scroll loading with pagination
- Date separators (Today / Yesterday / Weekday / Date)
- JSON export/import with duplicate detection

### UI/UX
- Two-panel layout: contact sidebar + chat panel
- File transfer panel with progress bars
- Online/offline status indicators with unread badges
- System native notifications (Dock badge, tray icon)
- Dark/light/auto theme (CSS variables + Tailwind)
- System tray with show/hide/quit

---

## Protocol Compatibility

| Client | Text | Files | Emoji | Shake |
|--------|:---:|:---:|:---:|:---:|
| 飞鸽传书 (IPMSG) | ✅ | ✅ | — | — |
| 飞秋 (FeiQ) Windows | ✅ | ✅ | ✅ | ✅ |
| 飞秋 (FeiQ) Mac | ✅ | ✅ | ✅ | ✅ |
| feiq++ ↔ feiq++ | ✅ | ✅ | ✅ | ✅ |

feiq++ version string: `feiq_plus_plus#128#MAC#0#0#0#1#9`

feiq++ peers additionally support end-to-end encrypted communication.

---

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Protocol Engine | Rust + tokio (async) |
| Encoding | encoding_rs (Mozilla, pure Rust GBK ↔ UTF-8) |
| Encryption | ring 0.17 (AES-256-GCM + x25519 ECDH) |
| Storage | rusqlite (bundled SQLite, no system dependency) |
| Desktop Shell | Tauri 2.11 |
| Frontend | React 18 + TypeScript + Vite |
| Styling | Tailwind CSS 3 |
| State | Zustand 4 |
| Relay Server | tokio-tungstenite + WebSocket |

---

## Project Stats

| Metric | Count |
|--------|:-----:|
| Rust source lines | ~8,100 |
| TypeScript source lines | ~3,500 |
| Rust source files | 34 |
| Rust unit tests | 77 (all passing) |
| TypeScript tests | 18 (all passing) |
| Tauri IPC commands | 27 |
| Protocol parser handlers | 14 (chain-of-responsibility) |
| Frontend components | 10 |
| Zustand stores | 4 |
| Emoji definitions | 96 |
| Cargo workspace crates | 3 (feiq-core, feiq-app, feiq-relay) |

---

## Installation

### macOS
- `feiq-plus-plus_0.1.4_x64.dmg` (Intel)
- `feiq-plus-plus_0.1.4_aarch64.dmg` (Apple Silicon)

### Windows
- `feiq-plus-plus_0.1.4_x64-setup.exe` / `.msi`

### Linux
- `feiq-plus-plus_0.1.4_amd64.deb`
- `feiq-plus-plus_0.1.4_amd64.AppImage`

### From Source
```bash
git clone https://github.com/lulyon/feiq-plus-plus.git
cd feiq-plus-plus
npm --prefix packages/feiq-gui install
cargo build --workspace --release
```

---

## Configuration

Auto-creates `~/.feiq_setting.ini` on first launch (compatible with original feiq format):

```ini
[user]
name = Your Name
host = Your Host

[network]
custom_group = 192.168.74.|192.168.82.

[app]
send_by_enter = 1
send_by_enter = 1
theme = auto

[relay]
mode = hybrid
server_url = ws://your-server:2426
room = default
```

---

## Known Limitations

- Image protocol: IPMSG_SENDIMAGE (0xC0) only provides 8-char image ID; actual data channel not reverse-engineered. Images sent as file attachments instead.
- Voice chat, remote desktop, scheduling — out of scope for this release
- IPMSG v9 legacy encryption (RSA/RC2/Blowfish) not implemented — feiq++ uses modern ECDH+AES instead
- Folder transfer deferred to future release
- 15 IPMSG commands (BR_ABSENCE, INPUTING, GETINFO, etc.) have no parser handlers — non-essential for core chat functionality

---

## Changelog

### v0.1.4 (Current)
- Relay server with hybrid LAN+Relay mode
- End-to-end encryption (ECDH + AES-256-GCM)
- Screenshot capture + Canvas annotation
- Theme system (light/dark/auto)
- Group chat (P2P dispatch)
- Full-text chat history search
- 100-agent security audit (17 issues fixed)
- AES-GCM nonce reuse fix (CRITICAL)
- Relay offline queue stable peer_key fix (CRITICAL)
- Engine stop UDP task leak fix
- Relay persistent connection fix (was opening new WS per message)
- Fellow::update() 6-field propagation fix
- TCP file size validation
- drain_pending transaction atomicity
- Frontend send_by_enter setting respected
- Frontend listener cleanup and dialog error handling

### v0.1.0 (Initial)
- Core IPMSG v9 protocol engine
- UDP LAN discovery + TCP file transfer
- Text messaging + emoji + knock
- Contact management with SQLite storage
- Cross-platform Tauri 2 desktop shell
- React frontend with Tailwind CSS

---

**95 tests · 0 errors · ~8,100 lines Rust · ~3,500 lines TypeScript**  
Built with Rust, Tauri, and React.  
[Full Implementation Plan →](https://github.com/lulyon/feiq-plus-plus/blob/main/docs/PLAN.md)

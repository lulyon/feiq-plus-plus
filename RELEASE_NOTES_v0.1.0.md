# feiq++ v0.1.0 — Initial Release

The first release of **feiq++**, a modern cross-platform LAN instant messaging app.  
Implements the full IP Messenger (飞鸽传书) Draft-9 protocol with FeiQ (飞秋) extensions.  
Built with **Rust + Tauri 2 + React**, delivering native performance with a modern UI.

---

## Highlights

- **Zero-server P2P architecture** — auto-discovers LAN peers via UDP broadcast (port 2425)
- **Full IPMSG v9 protocol** — complete command/option set including encryption flags
- **FeiQ protocol extensions** — 96 QQ-style emoji, window shake, typing indicators, custom broadcast
- **Cross-platform** — macOS (Intel + Apple Silicon), Windows, Linux
- **End-to-end encryption** — ECDH (x25519) + AES-256-GCM between feiq++ peers
- **27 unit tests** passing, zero compile errors

---

## Features

### Messaging
- Text messages with instant send/receive
- 96 built-in QQ-style emoji (with inline rendering in chat)
- Window shake / knock
- Configurable Enter / Ctrl+Enter to send
- Offline message queue (auto-delivered when peer comes online)

### File Transfer
- Single file & folder transfer
- 4GB+ large file support
- Real-time progress bar with transfer speed
- Drag-and-drop file sending
- Resume (offset-based continuation)
- Chunk size: 64KB (feiq++ ↔ feiq++) / 2KB (legacy compatible)

### Contacts & Groups
- Auto-discovery of LAN users
- Contact grouping with custom aliases
- Pinyin initial search
- Custom broadcast IP ranges (for cross-subnet discovery)
- P2P group chat (no server needed)

### Security
- ECDH (x25519) key exchange + AES-256-GCM encryption
- Only activated between feiq++ peers (detected via version string)
- Legacy clients communicate in plaintext

### Chat History
- SQLite persistent storage with full-text search
- Infinite scroll loading with pagination
- JSON export/import

### UI/UX
- Clean two-panel layout: contact list + chat panel
- Online/offline status indicators with unread badges
- System native notifications (Dock badge, tray icon)
- Dark/light theme ready (CSS variables)

---

## Protocol Compatibility

| Client | Text | Files | Emoji | Shake |
|--------|:---:|:---:|:---:|:---:|
| 飞鸽传书 (IPMSG) | ✅ | ✅ | — | — |
| 飞秋 (FeiQ) Windows | ✅ | ✅ | ✅ | ✅ |
| 飞秋 (FeiQ) Mac | ✅ | ✅ | ✅ | ✅ |
| feiq++ ↔ feiq++ | ✅ | ✅ | ✅ | ✅ |

feiq++ version string: `feiq_plus_plus#128#MAC#0#0#0#1#9`

---

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Protocol Engine | Rust + tokio (async) |
| Encoding | encoding_rs (Mozilla, pure Rust GBK ↔ UTF-8) |
| Encryption | ring 0.17 (AES-256-GCM + x25519) |
| Storage | rusqlite (bundled SQLite) |
| Desktop Shell | Tauri 2.11 |
| Frontend | React 18 + TypeScript + Vite |
| Styling | Tailwind CSS 4 |
| State | Zustand 4 |

---

## Project Stats

| Metric | Count |
|--------|:-----:|
| Rust source lines | 2,992 |
| TypeScript source lines | 749 |
| Unit tests | 27 (all passing) |
| Tauri IPC commands | 9 |
| Protocol handlers | 12 |
| Emoji definitions | 96 |

---

## Installation

### macOS
- `feiq-plus-plus_0.1.0_x64.dmg` (Intel)
- `feiq-plus-plus_0.1.0_aarch64.dmg` (Apple Silicon)

### Windows
- `feiq-plus-plus_0.1.0_x64-setup.exe` / `.msi`

### Linux
- `feiq-plus-plus_0.1.0_amd64.deb`
- `feiq-plus-plus_0.1.0_amd64.AppImage`

### From Source
```bash
git clone https://github.com/lulyon/feiq-plus-plus.git
cd feiq-plus-plus
npm --prefix packages/feiq-gui install
cargo build --workspace --release
```

---

## Configuration

Create `~/.feiq_setting.ini` (compatible with original feiq format):

```ini
[user]
name = Your Name
host = Your Host

[network]
custom_group = 192.168.74.|192.168.82.

[app]
send_by_enter = 1
```

---

## Known Limitations

- Image protocol: IPMSG_SENDIMAGE (0xC0) only provides 8-char image ID; actual data channel not reverse-engineered. Images sent as file attachments instead.
- Voice chat, remote desktop, scheduling — out of scope for this release
- IPMSG v9 legacy encryption (RSA/RC2/Blowfish) not implemented — feiq++ uses modern ECDH+AES instead

---

## What's Next

- [ ] Voice messaging support
- [ ] Full image protocol (reverse-engineer or custom extension)
- [ ] Speed limit control for file transfers
- [ ] File sharing service with password protection
- [ ] IPMSG v9 legacy encryption compatibility
- [ ] Mobile companion app

---

**27 tests · 0 errors · 2,992 lines Rust · 749 lines TypeScript**  
Built with Rust, Tauri, and React.  
[Full Implementation Plan →](https://github.com/lulyon/feiq-plus-plus/blob/main/PLAN.md)

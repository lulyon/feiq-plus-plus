# feiq-gui

feiq++ React frontend. LAN chat UI with contact sidebar, chat panel, emoji picker, and file transfer dialogs.

## Tech

- React 18 + TypeScript
- Vite 5 (build)
- Tailwind CSS 3 (styling)
- Zustand 4 (state management)
- Lucide React (icons)
- Tauri 2 API (IPC bridge to Rust backend)

## Development

```bash
npm install
npm run dev        # starts Vite on port 5173
cargo tauri dev    # starts Tauri with hot-reload
```

## Component Tree

```
App
├── Sidebar
│   ├── Search input
│   └── ContactItem[] (online dot, name, IP, unread badge)
├── ChatPanel
│   ├── ChatHeader (contact name, online status)
│   ├── MessageBubble[] (text, file, knock, emoji-rendered)
│   └── InputArea (+ EmojiPicker popup)
└── SettingsDialog (modal)
```

## Tauri Events (Rust → Frontend)

| Event | Payload | When |
|-------|---------|------|
| `contact-update` | `Fellow` | Contact online/offline/name change |
| `new-message` | `{fromIp, fromName, contents[], timestamp}` | New message received |
| `file-progress` | `{taskId, progress, total}` | File transfer progress |
| `file-state-changed` | `{taskId, state, message}` | File task finished/error/canceled |
| `send-timeout` | `{toIp, content}` | Message not confirmed by peer |

## Tauri Commands (Frontend → Rust)

| Command | Args | Returns |
|---------|------|---------|
| `start_engine` | — | Status string |
| `stop_engine` | — | Status string |
| `get_contacts` | — | `Fellow[]` |
| `search_contacts` | `query: string` | `Fellow[]` |
| `add_contact` | `ip: string` | `Fellow` |
| `get_settings` | — | `AppConfig` |
| `update_settings` | `config: AppConfig` | — |
| `get_emoji_list` | — | `EmojiInfo[]` |
| `send_knock` | `ip: string` | — |

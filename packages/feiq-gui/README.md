# feiq-gui

feiq++ React frontend. LAN + Relay chat UI with contact sidebar, chat panel, emoji picker, file transfer, screenshot annotation, and settings.

## Tech

- React 18 + TypeScript
- Vite 5 (build)
- Tailwind CSS 3 (styling)
- Zustand 4 (state management)
- Lucide React (icons)
- @radix-ui/react-dialog / popover / progress / tabs / toast / tooltip
- Tauri 2 API (IPC bridge to Rust backend)
- @tauri-apps/plugin-dialog / fs / notification

## Development

```bash
npm install
npm run dev        # starts Vite on port 5173
npm test           # runs 18 vitest tests
cargo tauri dev    # starts Tauri with hot-reload (from workspace root)
```

## Component Tree

```
App
├── Sidebar
│   ├── Search input
│   ├── Contact groups (CollapsibleGroup[])
│   │   └── ContactItem[] (online dot, name/IP, signature, unread badge, context menu)
│   └── Group list (selected group highlight)
├── ChatPanel / GroupChatPanel
│   ├── ChatHeader (contact name, online status)
│   ├── MessageBubble[] (text, emoji-rendered, file, knock, sealed)
│   │   └── DateSeparator (Today / Yesterday / weekday / date)
│   ├── Infinite scroll history loading
│   ├── Search panel (full-text history search)
│   └── InputArea
│       ├── Text input (Enter/Ctrl+Enter send)
│       ├── EmojiPicker popup (16×6 grid with hover preview)
│       ├── Screenshot button → ScreenshotAnnotation (Canvas overlay portal)
│       └── File attachment button
├── FileTransferPanel (collapsible, progress bars)
├── SettingsDialog (modal)
│   ├── Personal info (name, host)
│   ├── Connection mode (LAN/Relay/Hybrid + relay URL + room)
│   ├── Theme selector (Auto/Light/Dark)
│   ├── File sharing settings
│   └── Export/Import history
└── CreateGroupDialog (modal, member multi-select)
```

## Zustand Stores

| Store | File | State |
|-------|------|-------|
| `useContactStore` | `contactStore.ts` | `contacts[]`, `selectedIp`, upsert/select/setContacts |
| `useMessageStore` | `messageStore.ts` | `messagesByIp`, `unreadByIp`, `hasHistory`, `loadingHistory`, `historyOffset` |
| `useFileTransferStore` | `fileTransferStore.ts` | `transfers{}`, upsert/remove/activeTransfers |
| `useGroupStore` | `groupStore.ts` | `groups[]`, `selectedGroupName`, setGroups/selectGroup/addGroup |

## Tauri Events (Rust → Frontend)

| Event | Payload | When |
|-------|---------|------|
| `contact-update` | `Fellow` | Contact online/offline/name/alias change |
| `new-message` | `{fromIp, fromName, contents[], timestamp}` | New message received |
| `file-progress` | `{taskId, progress, total}` | File transfer progress update |
| `file-state-changed` | `{taskId, state, message}` | File task finished/error/canceled |
| `send-timeout` | `{toIp, content}` | Message not confirmed by peer |
| `engine-error` | `string` | Engine-level error |

## Tauri Commands (Frontend → Rust)

### Engine Lifecycle
| Command | Args | Returns |
|---------|------|---------|
| `start_engine` | — | Status |
| `stop_engine` | — | Status |

### Contacts
| Command | Args | Returns |
|---------|------|---------|
| `get_contacts` | — | `Fellow[]` |
| `search_contacts` | `query: string` | `Fellow[]` |
| `add_contact` | `ip: string` | `Fellow` |
| `set_alias` | `ip, alias` | — |
| `set_contact_group` | `ip, groupName` | — |

### Messaging
| Command | Args | Returns |
|---------|------|---------|
| `send_text` | `ip, text` | — |
| `send_knock` | `ip` | — |
| `get_emoji_list` | — | `EmojiInfo[]` |

### File Transfer
| Command | Args | Returns |
|---------|------|---------|
| `send_file` | `ip, filePath` | — |
| `download_file` | `taskId, savePath` | — |
| `cancel_file_task` | `taskId` | — |

### Groups
| Command | Args | Returns |
|---------|------|---------|
| `create_group` | `name, memberIps[]` | — |
| `get_groups` | — | `Group[]` |
| `send_group_text` | `groupName, text` | — |

### Chat History
| Command | Args | Returns |
|---------|------|---------|
| `get_chat_history` | `contactIp, offset, limit` | `MessageRecord[]` |
| `search_chat_history` | `query, limit` | `MessageRecord[]` |
| `export_history` | `filePath` | — |
| `import_history` | `filePath` | `count` |

### Settings
| Command | Args | Returns |
|---------|------|---------|
| `get_settings` | — | `AppConfig` |
| `update_settings` | `config` | — |

### Blacklist
| Command | Args | Returns |
|---------|------|---------|
| `get_blacklist` | — | `string[]` |
| `add_to_blacklist` | `ip` | — |
| `remove_from_blacklist` | `ip` | — |

### Other
| Command | Args | Returns |
|---------|------|---------|
| `capture_screenshot` | — | `filePath \| "FALLBACK"` |
| `reset_unread_count` | `ip` | — |

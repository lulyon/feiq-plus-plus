import { useEffect, useState, Fragment } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useContactStore } from "../stores/contactStore";
import { useMessageStore, type Message, type Content } from "../stores/messageStore";
import { MessageBubble } from "./MessageBubble";
import { InputArea } from "./InputArea";
import { MessageSquare } from "lucide-react";

/** Rust MessageRecord shape from get_chat_history */
interface MessageRecord {
  id: number;
  contact_ip: string;
  contact_name: string;
  direction: number; // 0 = sent, 1 = received
  content_json: string;
  timestamp: number;
}

/** Return a human-readable date label for the given timestamp */
function getDateLabel(timestamp: number): string {
  const date = new Date(timestamp);
  const now = new Date();
  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  const yesterday = new Date(today);
  yesterday.setDate(yesterday.getDate() - 1);
  const msgDate = new Date(date.getFullYear(), date.getMonth(), date.getDate());

  const diffDays = Math.floor(
    (today.getTime() - msgDate.getTime()) / (1000 * 60 * 60 * 24)
  );

  if (diffDays === 0) return "Today";
  if (diffDays === 1) return "Yesterday";
  if (diffDays < 7) {
    const days = [
      "Sunday", "Monday", "Tuesday", "Wednesday",
      "Thursday", "Friday", "Saturday",
    ];
    return days[date.getDay()];
  }
  return date.toLocaleDateString();
}

/** A styled date separator shown between messages on different days */
function DateSeparator({ label }: { label: string }) {
  return (
    <div className="flex items-center gap-3 py-2">
      <div className="flex-1 h-px bg-border" />
      <span className="text-xs text-text-muted flex-shrink-0">{label}</span>
      <div className="flex-1 h-px bg-border" />
    </div>
  );
}

export function ChatPanel() {
  const selectedIp = useContactStore((s) => s.selectedIp);
  const contacts = useContactStore((s) => s.contacts);
  const messagesByIp = useMessageStore((s) => s.messagesByIp);
  const prependMessages = useMessageStore((s) => s.prependMessages);
  const hasHistory = useMessageStore((s) => s.hasHistory);
  const setHasHistory = useMessageStore((s) => s.setHasHistory);

  const fellow = contacts.find((c) => c.ip === selectedIp);
  const displayName = fellow
    ? fellow.alias || fellow.name || fellow.pc_name || fellow.ip
    : "";
  const messages = selectedIp ? messagesByIp[selectedIp] || [] : [];

  // Alias inline editing state
  const [editingAlias, setEditingAlias] = useState(false);
  const [aliasDraft, setAliasDraft] = useState("");

  const handleAliasDoubleClick = () => {
    setAliasDraft(displayName === fellow?.ip ? "" : displayName);
    setEditingAlias(true);
  };

  const handleAliasSave = async () => {
    if (!selectedIp) return;
    try {
      await invoke("set_alias", { ip: selectedIp, alias: aliasDraft });
    } catch (e) {
      console.error("set_alias failed:", e);
    }
    setEditingAlias(false);
  };

  // Load chat history when switching contacts
  useEffect(() => {
    if (!selectedIp) return;
    if (hasHistory[selectedIp]) return; // already loaded

    invoke<MessageRecord[]>("get_chat_history", {
      ip: selectedIp,
      offset: 0,
      limit: 100,
    })
      .then((records) => {
        if (records.length === 0) {
          setHasHistory(selectedIp, true);
          return;
        }
        const msgs: Message[] = records.map((r) => ({
          fromIp: r.contact_ip,
          fromName: r.contact_name,
          contents: parseContentJson(r.content_json),
          timestamp: r.timestamp,
          direction: r.direction === 0 ? "sent" : "received",
        }));
        prependMessages(selectedIp, msgs);
        setHasHistory(selectedIp, true);
      })
      .catch((e) => console.error("Failed to load chat history:", e));
  }, [selectedIp]);

  /** Parse JSON content array from Rust (externally-tagged serde enum) into Content[] */
  function parseContentJson(json: string): Content[] {
    try {
      const parsed = JSON.parse(json);
      if (Array.isArray(parsed)) {
        return parsed.map((item: Record<string, unknown>) => {
          // Handle externally-tagged format: {"text": {"text": "Hello", "format": ""}}
          if (item.text !== undefined && typeof item.text === "object") {
            const inner = item.text as Record<string, unknown>;
            return { type: "text" as const, text: String(inner.text || "") };
          }
          if (item.knock !== undefined) {
            return { type: "knock" as const };
          }
          if (item.file !== undefined && typeof item.file === "object") {
            const inner = item.file as Record<string, unknown>;
            return {
              type: "file" as const,
              filename: String(inner.filename || ""),
              size: Number(inner.size || 0),
            };
          }
          // Handle internally-tagged format (frontend-sent): {"type": "text", "text": "Hello"}
          if (item.type === "text") {
            return { type: "text" as const, text: String(item.text || "") };
          }
          if (item.type === "knock") {
            return { type: "knock" as const };
          }
          if (item.type === "file") {
            return {
              type: "file" as const,
              filename: String(item.filename || ""),
              size: Number(item.size || 0),
            };
          }
          return { type: "text" as const, text: String(item.text || "") };
        });
      }
    } catch {
      // If parsing fails, treat as plain text
    }
    return [{ type: "text" as const, text: json }];
  }

  if (!selectedIp || !fellow) {
    return (
      <div className="flex-1 flex items-center justify-center bg-surface">
        <div className="text-center text-text-muted">
          <MessageSquare className="w-16 h-16 mx-auto mb-4 opacity-30" />
          <p className="text-lg">feiq++</p>
          <p className="text-sm mt-1">Select a contact to start chatting</p>
          <p className="text-xs mt-4 text-text-muted">
            LAN instant messaging · IP Messenger compatible
          </p>
        </div>
      </div>
    );
  }

  return (
    <div className="flex-1 flex flex-col bg-surface">
      {/* Chat Header */}
      <div className="px-4 py-3 border-b border-border flex items-center gap-3 bg-surface-alt">
        <span
          className={`w-2.5 h-2.5 rounded-full ${
            fellow.online ? "bg-online" : "bg-offline"
          }`}
        />
        <div className="flex-1 min-w-0">
          {editingAlias ? (
            <input
              type="text"
              value={aliasDraft}
              onChange={(e) => setAliasDraft(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") handleAliasSave();
                if (e.key === "Escape") setEditingAlias(false);
              }}
              onBlur={() => setEditingAlias(false)}
              className="text-sm font-semibold w-full px-1 py-0.5 border border-border rounded
                         focus:outline-none focus:border-primary bg-surface"
              autoFocus
            />
          ) : (
            <div
              className="text-sm font-semibold text-text cursor-pointer"
              onDoubleClick={handleAliasDoubleClick}
              title="Double-click to edit alias"
            >
              {displayName}
            </div>
          )}
          {fellow.signature && !editingAlias && (
            <div className="text-xs text-text-muted truncate">{fellow.signature}</div>
          )}
          <div className="text-xs text-text-muted">
            {fellow.online ? "Online" : "Offline"} · {fellow.ip}
          </div>
        </div>
      </div>

      {/* Messages */}
      <div className="flex-1 overflow-y-auto px-4 py-3 space-y-2">
        {messages.length === 0 ? (
          <div className="text-center text-text-muted text-sm mt-8">
            No messages yet. Say hello!
          </div>
        ) : (() => {
          let lastDate: string | null = null;
          return messages.map((msg, i) => {
            const msgDate = getDateLabel(msg.timestamp);
            const showSeparator = msgDate !== lastDate;
            lastDate = msgDate;
            return (
              <Fragment key={`${msg.timestamp}-${i}`}>
                {showSeparator && <DateSeparator label={msgDate} />}
                <MessageBubble message={msg} />
              </Fragment>
            );
          });
        })()}
      </div>

      {/* Input Area */}
      <InputArea fellowIp={fellow.ip} />
    </div>
  );
}

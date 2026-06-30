import { useEffect } from "react";
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
      <div className="flex-1 flex items-center justify-center bg-white">
        <div className="text-center text-gray-400">
          <MessageSquare className="w-16 h-16 mx-auto mb-4 opacity-30" />
          <p className="text-lg">feiq++</p>
          <p className="text-sm mt-1">Select a contact to start chatting</p>
          <p className="text-xs mt-4 text-gray-300">
            LAN instant messaging · IP Messenger compatible
          </p>
        </div>
      </div>
    );
  }

  return (
    <div className="flex-1 flex flex-col bg-white">
      {/* Chat Header */}
      <div className="px-4 py-3 border-b border-gray-200 flex items-center gap-3 bg-gray-50">
        <span
          className={`w-2.5 h-2.5 rounded-full ${
            fellow.online ? "bg-green-500" : "bg-gray-300"
          }`}
        />
        <div>
          <div className="text-sm font-semibold text-gray-800">{displayName}</div>
          <div className="text-xs text-gray-400">
            {fellow.online ? "Online" : "Offline"} · {fellow.ip}
          </div>
        </div>
      </div>

      {/* Messages */}
      <div className="flex-1 overflow-y-auto px-4 py-3 space-y-2">
        {messages.length === 0 ? (
          <div className="text-center text-gray-400 text-sm mt-8">
            No messages yet. Say hello!
          </div>
        ) : (
          messages.map((msg, i) => (
            <MessageBubble
              key={`${msg.timestamp}-${i}`}
              message={msg}
            />
          ))
        )}
      </div>

      {/* Input Area */}
      <InputArea fellowIp={fellow.ip} />
    </div>
  );
}

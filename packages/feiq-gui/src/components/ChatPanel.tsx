import { useContactStore } from "../stores/contactStore";
import { useMessageStore } from "../stores/messageStore";
import { MessageBubble } from "./MessageBubble";
import { InputArea } from "./InputArea";
import { MessageSquare } from "lucide-react";

export function ChatPanel() {
  const selectedIp = useContactStore((s) => s.selectedIp);
  const contacts = useContactStore((s) => s.contacts);
  const messagesByIp = useMessageStore((s) => s.messagesByIp);

  const fellow = contacts.find((c) => c.ip === selectedIp);
  const displayName = fellow
    ? fellow.alias || fellow.name || fellow.pc_name || fellow.ip
    : "";
  const messages = selectedIp ? messagesByIp[selectedIp] || [] : [];

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

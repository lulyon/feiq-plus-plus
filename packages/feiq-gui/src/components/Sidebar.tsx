import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useContactStore } from "../stores/contactStore";
import { useMessageStore } from "../stores/messageStore";
import { Users, Wifi, WifiOff, Plus } from "lucide-react";

export function Sidebar() {
  const contacts = useContactStore((s) => s.contacts);
  const selectedIp = useContactStore((s) => s.selectedIp);
  const selectContact = useContactStore((s) => s.selectContact);
  const upsertContact = useContactStore((s) => s.upsertContact);
  const unreadByIp = useMessageStore((s) => s.unreadByIp);
  const markRead = useMessageStore((s) => s.markRead);

  const [showAdd, setShowAdd] = useState(false);
  const [addValue, setAddValue] = useState("");
  const onlineCount = contacts.filter((c) => c.online).length;

  const handleAdd = async () => {
    const trimmed = addValue.trim();
    if (!trimmed) return;
    // Parse IP[:port] format, e.g. "127.0.0.1:2426"
    const parts = trimmed.split(":");
    const ip = parts[0];
    const port = parts[1] ? parseInt(parts[1], 10) : 2425;

    try {
      await invoke("add_contact", { ip }); // or add_contact_with_port
      upsertContact({ ip, port, pc_name: "", name: ip, host: "", mac: "", online: true, version: "", alias: "", group_name: "", signature: "" });
      setAddValue("");
      setShowAdd(false);
    } catch (e) {
      console.error("Add contact failed:", e);
    }
  };

  return (
    <div className="w-64 bg-white border-r border-gray-200 flex flex-col flex-shrink-0">
      {/* Header */}
      <div className="px-4 py-3 border-b border-gray-100">
        <div className="flex items-center justify-between">
          <h1 className="text-lg font-bold text-blue-600">feiq++</h1>
          <span className="text-xs text-gray-400">
            <Users className="w-3 h-3 inline mr-1" />
            {onlineCount}/{contacts.length}
          </span>
        </div>
      </div>

      {/* Search */}
      <div className="px-3 py-2 flex gap-1">
        <input
          type="text"
          placeholder="Search contacts..."
          className="flex-1 text-sm px-2 py-1.5 rounded-md border border-gray-200
                     focus:outline-none focus:border-blue-400 bg-gray-50"
        />
        <button
          onClick={() => setShowAdd(!showAdd)}
          className="w-7 h-7 flex items-center justify-center rounded-md hover:bg-gray-200 text-gray-500 cursor-pointer"
        >
          <Plus className="w-4 h-4" />
        </button>
      </div>

      {/* Add contact form */}
      {showAdd && (
        <div className="px-3 pb-2">
          <input
            type="text"
            value={addValue}
            onChange={(e) => setAddValue(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && handleAdd()}
            placeholder="IP:port (e.g. 127.0.0.1:2426)"
            className="w-full text-xs px-2 py-1 rounded-md border border-blue-300
                       focus:outline-none focus:border-blue-500 bg-white"
            autoFocus
          />
        </div>
      )}

      {/* Contact List */}
      <div className="flex-1 overflow-y-auto">
        {contacts.length === 0 ? (
          <div className="px-4 py-8 text-center text-sm text-gray-400">
            <WifiOff className="w-8 h-8 mx-auto mb-2 opacity-50" />
            <p>No contacts found</p>
            <p className="text-xs mt-1">Waiting for LAN discovery...</p>
          </div>
        ) : (
          contacts.map((fellow) => {
            const isSelected = fellow.ip === selectedIp;
            const unread = unreadByIp[fellow.ip] || 0;
            const displayName = fellow.alias || fellow.name || fellow.pc_name || fellow.ip;

            return (
              <div
                key={fellow.ip}
                onClick={() => {
                  selectContact(fellow.ip);
                  markRead(fellow.ip);
                }}
                className={`flex items-center gap-3 px-4 py-2.5 cursor-pointer border-l-3 transition-colors
                  ${isSelected
                    ? "bg-blue-50 border-l-blue-500"
                    : "border-l-transparent hover:bg-gray-50"
                  }`}
              >
                {/* Status dot */}
                <span
                  className={`w-2.5 h-2.5 rounded-full flex-shrink-0 ${
                    fellow.online ? "bg-green-500" : "bg-gray-300"
                  }`}
                />

                {/* Name + IP */}
                <div className="flex-1 min-w-0">
                  <div className="text-sm font-medium text-gray-800 truncate">
                    {displayName}
                  </div>
                  <div className="text-xs text-gray-400 truncate">{fellow.ip}</div>
                </div>

                {/* Unread badge */}
                {unread > 0 && (
                  <span className="bg-red-500 text-white text-xs rounded-full
                                   min-w-[20px] h-5 flex items-center justify-center px-1">
                    {unread > 99 ? "99+" : unread}
                  </span>
                )}

                {/* Online indicator */}
                {fellow.online && (
                  <Wifi className="w-3 h-3 text-green-500 flex-shrink-0" />
                )}
              </div>
            );
          })
        )}
      </div>
    </div>
  );
}

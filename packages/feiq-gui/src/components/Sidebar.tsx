import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useContactStore } from "../stores/contactStore";
import { useMessageStore } from "../stores/messageStore";
import { Users, Wifi, WifiOff, Plus, Cloud } from "lucide-react";

export function Sidebar() {
  const contacts = useContactStore((s) => s.contacts);
  const selectedIp = useContactStore((s) => s.selectedIp);
  const selectContact = useContactStore((s) => s.selectContact);
  const upsertContact = useContactStore((s) => s.upsertContact);
  const unreadByIp = useMessageStore((s) => s.unreadByIp);
  const markRead = useMessageStore((s) => s.markRead);

  const [showAdd, setShowAdd] = useState(false);
  const [addValue, setAddValue] = useState("");
  // Context menu state
  const [contextMenu, setContextMenu] = useState<{ ip: string; x: number; y: number } | null>(null);
  // Inline alias editing state
  const [editingAliasIp, setEditingAliasIp] = useState<string | null>(null);
  const [editingAliasValue, setEditingAliasValue] = useState("");
  const aliasInputRef = useRef<HTMLInputElement>(null);

  const onlineCount = contacts.filter((c) => c.online).length;

  // Close context menu on outside click
  useEffect(() => {
    if (!contextMenu) return;
    const close = () => setContextMenu(null);
    document.addEventListener("click", close);
    return () => document.removeEventListener("click", close);
  }, [contextMenu]);

  // Focus alias input when opened
  useEffect(() => {
    if (editingAliasIp) {
      aliasInputRef.current?.focus();
    }
  }, [editingAliasIp]);

  const handleContextMenu = (e: React.MouseEvent, ip: string) => {
    e.preventDefault();
    setContextMenu({ ip, x: e.clientX, y: e.clientY });
  };

  const handleEditAlias = (ip: string) => {
    const fellow = contacts.find((c) => c.ip === ip);
    setEditingAliasValue(
      fellow ? (fellow.alias || fellow.name || fellow.pc_name || fellow.ip) : ip
    );
    setEditingAliasIp(ip);
    setContextMenu(null);
  };

  const handleSidebarAliasSave = async () => {
    if (!editingAliasIp) return;
    try {
      await invoke("set_alias", { ip: editingAliasIp, alias: editingAliasValue });
    } catch (e) {
      console.error("set_alias failed:", e);
    }
    setEditingAliasIp(null);
    setEditingAliasValue("");
  };

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
    <div className="w-64 bg-surface border-r border-border flex flex-col flex-shrink-0">
      {/* Header */}
      <div className="px-4 py-3 border-b border-border">
        <div className="flex items-center justify-between">
          <h1 className="text-lg font-bold text-primary">feiq++</h1>
          <span className="text-xs text-text-muted">
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
          className="flex-1 text-sm px-2 py-1.5 rounded-md border border-border
                     focus:outline-none focus:border-primary bg-surface-alt"
        />
        <button
          onClick={() => setShowAdd(!showAdd)}
          className="w-7 h-7 flex items-center justify-center rounded-md hover:bg-surface-alt text-text-muted cursor-pointer"
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
            className="w-full text-xs px-2 py-1 rounded-md border border-primary
                       focus:outline-none focus:border-primary bg-surface"
            autoFocus
          />
        </div>
      )}

      {/* Contact List */}
      <div className="flex-1 overflow-y-auto">
        {contacts.length === 0 ? (
          <div className="px-4 py-8 text-center text-sm text-text-muted">
            <WifiOff className="w-8 h-8 mx-auto mb-2 opacity-50" />
            <p>No contacts found</p>
            <p className="text-xs mt-1">Waiting for LAN discovery...</p>
          </div>
        ) : (
          contacts.map((fellow) => {
            const isSelected = fellow.ip === selectedIp;
            const unread = unreadByIp[fellow.ip] || 0;
            const displayName = fellow.alias || fellow.name || fellow.pc_name || fellow.ip;
            const isEditing = editingAliasIp === fellow.ip;

            return (
              <div
                key={fellow.ip}
                onClick={() => {
                  if (isEditing) return;
                  selectContact(fellow.ip);
                  markRead(fellow.ip);
                }}
                onContextMenu={(e) => handleContextMenu(e, fellow.ip)}
                className={`flex items-center gap-3 px-4 py-2.5 cursor-pointer border-l-3 transition-colors
                  ${isSelected
                    ? "bg-primary/10 border-l-primary"
                    : "border-l-transparent hover:bg-surface-alt"
                  }`}
              >
                {/* Status dot */}
                <span
                  className={`w-2.5 h-2.5 rounded-full flex-shrink-0 ${
                    fellow.online ? "bg-online" : "bg-offline"
                  }`}
                />

                {/* Name + IP + Signature */}
                <div className="flex-1 min-w-0">
                  {isEditing ? (
                    <input
                      ref={aliasInputRef}
                      type="text"
                      value={editingAliasValue}
                      onChange={(e) => setEditingAliasValue(e.target.value)}
                      onKeyDown={(e) => {
                        e.stopPropagation();
                        if (e.key === "Enter") handleSidebarAliasSave();
                        if (e.key === "Escape") {
                          setEditingAliasIp(null);
                          setEditingAliasValue("");
                        }
                      }}
                      onBlur={() => {
                        // Small delay to allow click on context menu item
                        setTimeout(() => {
                          setEditingAliasIp(null);
                          setEditingAliasValue("");
                        }, 200);
                      }}
                      onClick={(e) => e.stopPropagation()}
                      className="text-sm font-medium w-full px-1 py-0.5 border border-border rounded
                                 focus:outline-none focus:border-primary bg-surface"
                    />
                  ) : (
                    <div className="text-sm font-medium text-text truncate">
                      {displayName}
                    </div>
                  )}
                  {fellow.signature && !isEditing && (
                    <div className="text-xs text-text-muted truncate">{fellow.signature}</div>
                  )}
                  <div className="text-xs text-text-muted truncate flex items-center gap-1">
                    {fellow.source && typeof fellow.source === "object" && "RelayPeer" in fellow.source ? (
                      <>
                        <Cloud className="w-3 h-3 text-primary flex-shrink-0" />
                        <span>Relay</span>
                      </>
                    ) : (
                      fellow.ip
                    )}
                  </div>
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
                  <Wifi className="w-3 h-3 text-online flex-shrink-0" />
                )}
              </div>
            );
          })
        )}
      </div>

      {/* Context Menu */}
      {contextMenu && (
        <div
          className="fixed z-50 bg-surface border border-border rounded-md shadow-lg py-1 text-sm"
          style={{ left: contextMenu.x, top: contextMenu.y }}
          onClick={(e) => e.stopPropagation()}
        >
          <button
            className="w-full text-left px-4 py-1.5 hover:bg-surface-alt text-text cursor-pointer"
            onClick={() => handleEditAlias(contextMenu.ip)}
          >
            Edit Alias
          </button>
        </div>
      )}
    </div>
  );
}

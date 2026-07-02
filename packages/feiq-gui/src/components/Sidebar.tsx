import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useContactStore } from "../stores/contactStore";
import { useMessageStore } from "../stores/messageStore";
import { useGroupStore } from "../stores/groupStore";
import { Users, Wifi, WifiOff, Plus, Cloud, ChevronRight } from "lucide-react";
import { CreateGroupDialog } from "./CreateGroupDialog";

/** A single collapsible group section */
function CollapsibleGroup({
  name,
  count,
  expanded,
  onToggle,
  children,
}: {
  name: string;
  count: number;
  expanded: boolean;
  onToggle: () => void;
  children: React.ReactNode;
}) {
  return (
    <div>
      <div
        onClick={onToggle}
        className="flex items-center px-3 py-2 cursor-pointer hover:bg-surface-alt text-text-muted text-xs font-medium select-none"
      >
        <ChevronRight
          className={`w-3 h-3 mr-1 transition-transform ${expanded ? "rotate-90" : ""}`}
        />
        {name} ({count})
      </div>
      {expanded && <div>{children}</div>}
    </div>
  );
}

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
  // Search query
  // Groups
  const groups = useGroupStore((s) => s.groups);
  const selectedGroupName = useGroupStore((s) => s.selectedGroupName);
  const selectGroup = useGroupStore((s) => s.selectGroup);
  const [showCreateGroup, setShowCreateGroup] = useState(false);
  // Search query
  const [searchQuery, setSearchQuery] = useState("");
  // Track expanded groups
  const [expandedGroups, setExpandedGroups] = useState<Record<string, boolean>>({});
  // Blacklisted IPs set for block/unblock UI
  const [blacklistedIps, setBlacklistedIps] = useState<Set<string>>(new Set());

  const onlineCount = contacts.filter((c) => c.online).length;

  // Fetch blacklist on mount
  useEffect(() => {
    invoke<string[]>("get_blacklist")
      .then((list) => setBlacklistedIps(new Set(list)))
      .catch(console.error);
  }, []);

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

  const handleBlock = async (ip: string) => {
    try {
      await invoke("add_to_blacklist", { ip });
      setBlacklistedIps(new Set([...blacklistedIps, ip]));
    } catch (e) {
      console.error("Block failed:", e);
    }
    setContextMenu(null);
  };

  const handleUnblock = async (ip: string) => {
    try {
      await invoke("remove_from_blacklist", { ip });
      const newSet = new Set(blacklistedIps);
      newSet.delete(ip);
      setBlacklistedIps(newSet);
    } catch (e) {
      console.error("Unblock failed:", e);
    }
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

  /**
   * Filter contacts by search query, then group by group_name.
   * Returns array of { groupName, contacts } sorted with "Ungrouped" last.
   */
  const groupedContacts = (() => {
    // Apply search filter
    const filtered = searchQuery
      ? contacts.filter((c) => {
          const displayName = c.alias || c.name || c.pc_name || c.ip;
          const q = searchQuery.toLowerCase();
          return displayName.toLowerCase().includes(q) ||
                 c.ip.includes(q) ||
                 c.host.toLowerCase().includes(q) ||
                 c.pc_name.toLowerCase().includes(q);
        })
      : contacts;

    // Build groups
    const groupsMap = new Map<string, typeof filtered>();
    for (const fellow of filtered) {
      const groupName = fellow.group_name || "Ungrouped";
      if (!groupsMap.has(groupName)) {
        groupsMap.set(groupName, []);
      }
      groupsMap.get(groupName)!.push(fellow);
    }

    // Sort contacts within each group: online first, then by display name
    for (const [, members] of groupsMap) {
      members.sort((a, b) => {
        if (a.online !== b.online) return a.online ? -1 : 1;
        const nameA = (a.alias || a.name || a.pc_name || a.ip).toLowerCase();
        const nameB = (b.alias || b.name || b.pc_name || b.ip).toLowerCase();
        return nameA.localeCompare(nameB);
      });
    }

    // Convert to array, sorted by group name, "Ungrouped" last
    const groupEntries = Array.from(groupsMap.entries()).sort(([a], [b]) => {
      if (a === "Ungrouped") return 1;
      if (b === "Ungrouped") return -1;
      return a.localeCompare(b);
    });

    return groupEntries;
  })();

  /** Toggle a group's expanded state */
  const toggleGroup = (groupName: string) => {
    setExpandedGroups((prev) => ({
      ...prev,
      [groupName]: !prev[groupName],
    }));
  };

  /** Render a single contact item */
  const renderContact = (fellow: typeof contacts[0]) => {
    const isSelected = fellow.ip === selectedIp;
    const unread = unreadByIp[fellow.ip] || 0;
    const displayName = fellow.alias || fellow.name || fellow.pc_name || fellow.ip;
    const isEditing = editingAliasIp === fellow.ip;
    const isBlacklisted = blacklistedIps.has(fellow.ip);

    return (
      <div
        key={fellow.ip}
        onClick={() => {
          if (isEditing) return;
          selectGroup(null); // clear group selection
          selectContact(fellow.ip);
          markRead(fellow.ip);
          invoke("reset_unread_count").catch(() => {});
        }}
        onContextMenu={(e) => handleContextMenu(e, fellow.ip)}
        className={`flex items-center gap-3 px-4 py-2.5 cursor-pointer border-l-3 transition-colors
          ${isBlacklisted ? "opacity-50" : ""}
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
            <div className={`text-sm font-medium text-text truncate ${isBlacklisted ? "line-through" : ""}`}>
              {displayName}
            </div>
          )}
          {fellow.signature && !isEditing && (
            <div className={`text-xs text-text-muted truncate ${isBlacklisted ? "line-through" : ""}`}>{fellow.signature}</div>
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
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
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

      {/* Groups Section */}
      {groups.length > 0 && (
        <div className="border-t border-border">
          <CollapsibleGroup
            name="Groups"
            count={groups.length}
            expanded={expandedGroups["__groups__"] !== false}
            onToggle={() => toggleGroup("__groups__")}
          >
            {groups.map((group) => {
              const isGroupSelected = group.name === selectedGroupName;
              return (
                <div
                  key={group.name}
                  onClick={() => {
                    selectContact(null); // clear contact selection
                    selectGroup(group.name);
                    // No unread for groups yet — could add later
                  }}
                  className={`flex items-center gap-3 px-4 py-2.5 cursor-pointer border-l-3 transition-colors
                    ${isGroupSelected
                      ? "bg-primary/10 border-l-primary"
                      : "border-l-transparent hover:bg-surface-alt"
                    }`}
                >
                  <Users className="w-4 h-4 text-text-muted flex-shrink-0" />
                  <div className="flex-1 min-w-0">
                    <div className="text-sm font-medium text-text truncate">
                      {group.name}
                    </div>
                    <div className="text-xs text-text-muted">
                      {group.memberIps.length} members
                    </div>
                  </div>
                </div>
              );
            })}
            <button
              onClick={() => setShowCreateGroup(true)}
              className="flex items-center gap-2 px-4 py-2 w-full text-sm text-text-muted
                         hover:text-text hover:bg-surface-alt transition-colors cursor-pointer"
            >
              <Plus className="w-3.5 h-3.5" />
              Create Group
            </button>
          </CollapsibleGroup>
        </div>
      )}

      {/* Quick-create group button when no groups exist */}
      {groups.length === 0 && (
        <div className="px-4 py-1 border-t border-border">
          <button
            onClick={() => setShowCreateGroup(true)}
            className="flex items-center gap-2 w-full py-1.5 text-xs text-text-muted
                       hover:text-text transition-colors cursor-pointer"
          >
            <Plus className="w-3 h-3" />
            Create Group
          </button>
        </div>
      )}

      {/* CreateGroupDialog */}
      {showCreateGroup && (
        <CreateGroupDialog onClose={() => setShowCreateGroup(false)} />
      )}

      {/* Contact List (grouped tree view) */}
      <div className="flex-1 overflow-y-auto">
        {contacts.length === 0 ? (
          <div className="px-4 py-8 text-center text-sm text-text-muted">
            <WifiOff className="w-8 h-8 mx-auto mb-2 opacity-50" />
            <p>No contacts found</p>
            <p className="text-xs mt-1">Waiting for LAN discovery...</p>
          </div>
        ) : groupedContacts.length === 0 ? (
          <div className="px-4 py-8 text-center text-sm text-text-muted">
            <p>No contacts match your search</p>
          </div>
        ) : (
          groupedContacts.map(([groupName, members]) => {
            const isExpanded = expandedGroups[groupName] !== false; // default expanded

            // When searching, flatten all groups (no collapsible)
            if (searchQuery) {
              return (
                <div key={groupName}>
                  {members.map(renderContact)}
                </div>
              );
            }

            return (
              <CollapsibleGroup
                key={groupName}
                name={groupName}
                count={members.length}
                expanded={isExpanded}
                onToggle={() => toggleGroup(groupName)}
              >
                {members.map(renderContact)}
              </CollapsibleGroup>
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
          {blacklistedIps.has(contextMenu.ip) ? (
            <button
              className="w-full text-left px-4 py-1.5 hover:bg-surface-alt text-green-600 cursor-pointer"
              onClick={() => handleUnblock(contextMenu.ip)}
            >
              Unblock
            </button>
          ) : (
            <button
              className="w-full text-left px-4 py-1.5 hover:bg-surface-alt text-red-500 cursor-pointer"
              onClick={() => handleBlock(contextMenu.ip)}
            >
              Block
            </button>
          )}
        </div>
      )}
    </div>
  );
}

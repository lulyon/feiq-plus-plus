import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useContactStore } from "../stores/contactStore";
import { useGroupStore } from "../stores/groupStore";
import { X, Users } from "lucide-react";

interface Props {
  onClose: () => void;
}

export function CreateGroupDialog({ onClose }: Props) {
  const contacts = useContactStore((s) => s.contacts);
  const addGroup = useGroupStore((s) => s.addGroup);

  const [groupName, setGroupName] = useState("");
  const [selectedIps, setSelectedIps] = useState<Set<string>>(new Set());

  const toggleIp = (ip: string) => {
    setSelectedIps((prev) => {
      const next = new Set(prev);
      if (next.has(ip)) {
        next.delete(ip);
      } else {
        next.add(ip);
      }
      return next;
    });
  };

  const handleCreate = async () => {
    const name = groupName.trim();
    if (!name) return;

    const memberIps = Array.from(selectedIps);
    if (memberIps.length === 0) return;

    try {
      await invoke("create_group", { name, memberIps });
      addGroup({ name, memberIps });
      onClose();
    } catch (e) {
      console.error("create_group failed:", e);
    }
  };

  return (
    <div className="fixed inset-0 bg-overlay flex items-center justify-center z-50">
      <div className="bg-surface rounded-lg shadow-xl w-96 max-w-[90vw]">
        <div className="flex items-center justify-between px-4 py-3 border-b border-border">
          <h2 className="text-base font-semibold">Create Group</h2>
          <button
            onClick={onClose}
            className="p-1 hover:bg-surface-alt rounded cursor-pointer"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        <div className="px-4 py-3 space-y-3 max-h-[70vh] overflow-y-auto">
          <label className="block">
            <span className="text-sm text-text-muted">Group Name</span>
            <input
              type="text"
              value={groupName}
              onChange={(e) => setGroupName(e.target.value)}
              className="mt-1 w-full text-sm px-2 py-1.5 border border-border rounded-md
                         focus:outline-none focus:border-primary"
              placeholder="e.g. Team Alpha"
              autoFocus
            />
          </label>

          <div>
            <span className="text-sm text-text-muted block mb-1">
              Members ({selectedIps.size} selected)
            </span>
            <div className="max-h-48 overflow-y-auto border border-border rounded-md divide-y divide-border">
              {contacts.length === 0 ? (
                <div className="px-3 py-4 text-sm text-text-muted text-center">
                  No contacts available
                </div>
              ) : (
                contacts.map((fellow) => {
                  const displayName =
                    fellow.alias || fellow.name || fellow.pc_name || fellow.ip;
                  return (
                    <label
                      key={fellow.ip}
                      className="flex items-center gap-3 px-3 py-2 hover:bg-surface-alt cursor-pointer"
                    >
                      <input
                        type="checkbox"
                        checked={selectedIps.has(fellow.ip)}
                        onChange={() => toggleIp(fellow.ip)}
                        className="rounded"
                      />
                      <span
                        className={`w-2 h-2 rounded-full flex-shrink-0 ${
                          fellow.online ? "bg-online" : "bg-offline"
                        }`}
                      />
                      <span className="flex-1 text-sm text-text truncate">
                        {displayName}
                      </span>
                      <span className="text-xs text-text-muted">{fellow.ip}</span>
                    </label>
                  );
                })
              )}
            </div>
          </div>
        </div>

        <div className="px-4 py-3 border-t border-border flex justify-end gap-2">
          <button
            onClick={onClose}
            className="px-3 py-1.5 text-sm border border-border rounded-md hover:bg-surface-alt cursor-pointer"
          >
            Cancel
          </button>
          <button
            onClick={handleCreate}
            disabled={!groupName.trim() || selectedIps.size === 0}
            className="px-3 py-1.5 text-sm bg-primary text-white rounded-md
                       hover:bg-primary-hover flex items-center gap-1
                       disabled:bg-offline transition-colors cursor-pointer"
          >
            <Users className="w-3.5 h-3.5" />
            Create
          </button>
        </div>
      </div>
    </div>
  );
}

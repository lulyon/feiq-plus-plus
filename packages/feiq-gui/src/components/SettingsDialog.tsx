import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { save as dialogSave, open as dialogOpen } from "@tauri-apps/plugin-dialog";
import { X, Save, Download, Upload } from "lucide-react";

interface AppConfig {
  name: string;
  host: string;
  title: string;
  send_by_enter: boolean;
  custom_group: string;
  rank_user_enable: boolean;
  mode: "lan" | "relay" | "hybrid";
  relay_server_url: string;
  relay_room: string;
  theme: string;
}

interface Props {
  onClose: () => void;
}

const MODE_LABELS: Record<string, string> = {
  lan: "LAN Only",
  relay: "Relay Only",
  hybrid: "Hybrid (LAN + Relay)",
};

export function SettingsDialog({ onClose }: Props) {
  const [config, setConfig] = useState<AppConfig | null>(null);

  useEffect(() => {
    invoke<AppConfig>("get_settings")
      .then(setConfig)
      .catch(() =>
        setConfig({
          name: "",
          host: "feiq++",
          title: "feiq++",
          send_by_enter: true,
          custom_group: "",
          rank_user_enable: true,
          mode: "lan",
          relay_server_url: "",
          relay_room: "default",
          theme: "auto",
        })
      );
  }, []);

  const update = (key: keyof AppConfig, value: unknown) => {
    setConfig((prev) => (prev ? { ...prev, [key]: value } : null));
  };

  const save = async () => {
    if (!config) return;
    await invoke("update_settings", { config }).catch(console.error);
    onClose();
  };

  const handleExport = async () => {
    try {
      const filePath = await dialogSave({
        filters: [{ name: "JSON", extensions: ["json"] }],
        defaultPath: "feiq_history_export.json",
      });
      if (!filePath) return;
      await invoke("export_history", { path: filePath });
      alert("Chat history exported successfully.");
    } catch (e) {
      alert("Export failed: " + String(e));
    }
  };

  const handleImport = async () => {
    try {
      const filePath = await dialogOpen({
        filters: [{ name: "JSON", extensions: ["json"] }],
        multiple: false,
      });
      if (!filePath) return;
      const path = Array.isArray(filePath) ? filePath[0] : filePath;
      const count = await invoke<number>("import_history", { path });
      alert(`Imported ${count} messages successfully.`);
    } catch (e) {
      alert("Import failed: " + String(e));
    }
  };

  if (!config)
    return (
      <div className="fixed inset-0 bg-overlay flex items-center justify-center z-50">
        <div className="bg-surface rounded-lg p-6">Loading...</div>
      </div>
    );

  const needsRelay = config.mode === "relay" || config.mode === "hybrid";

  return (
    <div className="fixed inset-0 bg-overlay flex items-center justify-center z-50">
      <div className="bg-surface rounded-lg shadow-xl w-96 max-w-[90vw]">
        <div className="flex items-center justify-between px-4 py-3 border-b border-border">
          <h2 className="text-base font-semibold">Settings</h2>
          <button onClick={onClose} className="p-1 hover:bg-surface-alt rounded cursor-pointer">
            <X className="w-4 h-4" />
          </button>
        </div>

        <div className="px-4 py-3 space-y-3 max-h-[70vh] overflow-y-auto">
          <label className="block">
            <span className="text-sm text-text-muted">Display Name</span>
            <input
              type="text"
              value={config.name}
              onChange={(e) => update("name", e.target.value)}
              className="mt-1 w-full text-sm px-2 py-1.5 border border-border rounded-md focus:outline-none focus:border-primary"
              placeholder="Your name"
            />
          </label>
          <label className="block">
            <span className="text-sm text-text-muted">Host Name</span>
            <input
              type="text"
              value={config.host}
              onChange={(e) => update("host", e.target.value)}
              className="mt-1 w-full text-sm px-2 py-1.5 border border-border rounded-md focus:outline-none focus:border-primary"
            />
          </label>

          {/* Connection Mode */}
          <label className="block">
            <span className="text-sm text-text-muted">Connection Mode</span>
            <select
              value={config.mode}
              onChange={(e) => update("mode", e.target.value)}
              className="mt-1 w-full text-sm px-2 py-1.5 border border-border rounded-md focus:outline-none focus:border-primary bg-surface"
            >
              {Object.entries(MODE_LABELS).map(([val, label]) => (
                <option key={val} value={val}>{label}</option>
              ))}
            </select>
          </label>

          {/* LAN settings — always visible */}
          <label className="block">
            <span className="text-sm text-text-muted">Custom Broadcast IPs</span>
            <input
              type="text"
              value={config.custom_group}
              onChange={(e) => update("custom_group", e.target.value)}
              className="mt-1 w-full text-sm px-2 py-1.5 border border-border rounded-md focus:outline-none focus:border-primary"
              placeholder="e.g. 192.168.74.|192.168.82."
            />
            <span className="text-xs text-text-muted">
              End each segment with "." and separate with "|"
            </span>
          </label>

          {/* Relay settings — shown when mode is relay or hybrid */}
          {needsRelay && (
            <>
              <div className="border-t border-border pt-3">
                <span className="text-xs text-text-muted uppercase tracking-wide">
                  Relay Server
                </span>
              </div>
              <label className="block">
                <span className="text-sm text-text-muted">Server URL</span>
                <input
                  type="text"
                  value={config.relay_server_url}
                  onChange={(e) => update("relay_server_url", e.target.value)}
                  className="mt-1 w-full text-sm px-2 py-1.5 border border-border rounded-md focus:outline-none focus:border-primary"
                  placeholder="ws://your-server:2426"
                />
              </label>
              <label className="block">
                <span className="text-sm text-text-muted">Room Name</span>
                <input
                  type="text"
                  value={config.relay_room}
                  onChange={(e) => update("relay_room", e.target.value)}
                  className="mt-1 w-full text-sm px-2 py-1.5 border border-border rounded-md focus:outline-none focus:border-primary"
                  placeholder="default"
                />
              </label>
            </>
          )}

          <div className="border-t border-border pt-3">
            <label className="flex items-center gap-2">
              <input
                type="checkbox"
                checked={config.send_by_enter}
                onChange={(e) => update("send_by_enter", e.target.checked)}
                className="rounded"
              />
              <span className="text-sm text-text-muted">Enter to send message</span>
            </label>
            <label className="flex items-center gap-2 mt-2">
              <input
                type="checkbox"
                checked={config.rank_user_enable}
                onChange={(e) => update("rank_user_enable", e.target.checked)}
                className="rounded"
              />
              <span className="text-sm text-text-muted">Rank contacts by frequency</span>
            </label>
          </div>

          {/* Theme */}
          <div className="border-t border-border pt-3">
            <label className="block">
              <span className="text-sm font-medium text-text-muted mb-2 block">Theme</span>
              <select
                value={config.theme}
                onChange={(e) => update("theme", e.target.value)}
                className="w-full px-3 py-2 border border-border rounded-lg bg-surface text-text text-sm focus:outline-none focus:border-primary"
              >
                <option value="auto">System</option>
                <option value="light">Light</option>
                <option value="dark">Dark</option>
              </select>
            </label>
          </div>

          {/* Data: Export / Import */}
          <div className="border-t border-border pt-3">
            <span className="text-sm font-medium text-text-muted block mb-2">Chat History</span>
            <div className="flex gap-2">
              <button
                onClick={handleExport}
                className="flex-1 px-3 py-2 text-sm border border-border rounded-md
                           hover:bg-surface-alt flex items-center justify-center gap-1.5 cursor-pointer"
              >
                <Download className="w-3.5 h-3.5" />
                Export
              </button>
              <button
                onClick={handleImport}
                className="flex-1 px-3 py-2 text-sm border border-border rounded-md
                           hover:bg-surface-alt flex items-center justify-center gap-1.5 cursor-pointer"
              >
                <Upload className="w-3.5 h-3.5" />
                Import
              </button>
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
            onClick={save}
            className="px-3 py-1.5 text-sm bg-primary text-white rounded-md hover:bg-primary-hover flex items-center gap-1 cursor-pointer"
          >
            <Save className="w-3.5 h-3.5" />
            Save
          </button>
        </div>
      </div>
    </div>
  );
}

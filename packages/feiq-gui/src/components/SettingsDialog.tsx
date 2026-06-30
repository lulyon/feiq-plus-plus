import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { X, Save } from "lucide-react";

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

  if (!config)
    return (
      <div className="fixed inset-0 bg-black/30 flex items-center justify-center z-50">
        <div className="bg-white rounded-lg p-6">Loading...</div>
      </div>
    );

  const needsRelay = config.mode === "relay" || config.mode === "hybrid";

  return (
    <div className="fixed inset-0 bg-black/30 flex items-center justify-center z-50">
      <div className="bg-white rounded-lg shadow-xl w-96 max-w-[90vw]">
        <div className="flex items-center justify-between px-4 py-3 border-b">
          <h2 className="text-base font-semibold">Settings</h2>
          <button onClick={onClose} className="p-1 hover:bg-gray-100 rounded cursor-pointer">
            <X className="w-4 h-4" />
          </button>
        </div>

        <div className="px-4 py-3 space-y-3 max-h-[70vh] overflow-y-auto">
          <label className="block">
            <span className="text-sm text-gray-600">Display Name</span>
            <input
              type="text"
              value={config.name}
              onChange={(e) => update("name", e.target.value)}
              className="mt-1 w-full text-sm px-2 py-1.5 border rounded-md focus:outline-none focus:border-blue-400"
              placeholder="Your name"
            />
          </label>
          <label className="block">
            <span className="text-sm text-gray-600">Host Name</span>
            <input
              type="text"
              value={config.host}
              onChange={(e) => update("host", e.target.value)}
              className="mt-1 w-full text-sm px-2 py-1.5 border rounded-md focus:outline-none focus:border-blue-400"
            />
          </label>

          {/* Connection Mode */}
          <label className="block">
            <span className="text-sm text-gray-600">Connection Mode</span>
            <select
              value={config.mode}
              onChange={(e) => update("mode", e.target.value)}
              className="mt-1 w-full text-sm px-2 py-1.5 border rounded-md focus:outline-none focus:border-blue-400 bg-white"
            >
              {Object.entries(MODE_LABELS).map(([val, label]) => (
                <option key={val} value={val}>{label}</option>
              ))}
            </select>
          </label>

          {/* LAN settings — always visible */}
          <label className="block">
            <span className="text-sm text-gray-600">Custom Broadcast IPs</span>
            <input
              type="text"
              value={config.custom_group}
              onChange={(e) => update("custom_group", e.target.value)}
              className="mt-1 w-full text-sm px-2 py-1.5 border rounded-md focus:outline-none focus:border-blue-400"
              placeholder="e.g. 192.168.74.|192.168.82."
            />
            <span className="text-xs text-gray-400">
              End each segment with "." and separate with "|"
            </span>
          </label>

          {/* Relay settings — shown when mode is relay or hybrid */}
          {needsRelay && (
            <>
              <div className="border-t pt-3">
                <span className="text-xs text-gray-400 uppercase tracking-wide">
                  Relay Server
                </span>
              </div>
              <label className="block">
                <span className="text-sm text-gray-600">Server URL</span>
                <input
                  type="text"
                  value={config.relay_server_url}
                  onChange={(e) => update("relay_server_url", e.target.value)}
                  className="mt-1 w-full text-sm px-2 py-1.5 border rounded-md focus:outline-none focus:border-blue-400"
                  placeholder="ws://your-server:2426"
                />
              </label>
              <label className="block">
                <span className="text-sm text-gray-600">Room Name</span>
                <input
                  type="text"
                  value={config.relay_room}
                  onChange={(e) => update("relay_room", e.target.value)}
                  className="mt-1 w-full text-sm px-2 py-1.5 border rounded-md focus:outline-none focus:border-blue-400"
                  placeholder="default"
                />
              </label>
            </>
          )}

          <div className="border-t pt-3">
            <label className="flex items-center gap-2">
              <input
                type="checkbox"
                checked={config.send_by_enter}
                onChange={(e) => update("send_by_enter", e.target.checked)}
                className="rounded"
              />
              <span className="text-sm text-gray-600">Enter to send message</span>
            </label>
            <label className="flex items-center gap-2 mt-2">
              <input
                type="checkbox"
                checked={config.rank_user_enable}
                onChange={(e) => update("rank_user_enable", e.target.checked)}
                className="rounded"
              />
              <span className="text-sm text-gray-600">Rank contacts by frequency</span>
            </label>
          </div>
        </div>

        <div className="px-4 py-3 border-t flex justify-end gap-2">
          <button
            onClick={onClose}
            className="px-3 py-1.5 text-sm border rounded-md hover:bg-gray-50 cursor-pointer"
          >
            Cancel
          </button>
          <button
            onClick={save}
            className="px-3 py-1.5 text-sm bg-blue-500 text-white rounded-md hover:bg-blue-600 flex items-center gap-1 cursor-pointer"
          >
            <Save className="w-3.5 h-3.5" />
            Save
          </button>
        </div>
      </div>
    </div>
  );
}

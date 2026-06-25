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
}

interface Props {
  onClose: () => void;
}

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

  return (
    <div className="fixed inset-0 bg-black/30 flex items-center justify-center z-50">
      <div className="bg-white rounded-lg shadow-xl w-96 max-w-[90vw]">
        <div className="flex items-center justify-between px-4 py-3 border-b">
          <h2 className="text-base font-semibold">Settings</h2>
          <button onClick={onClose} className="p-1 hover:bg-gray-100 rounded cursor-pointer">
            <X className="w-4 h-4" />
          </button>
        </div>

        <div className="px-4 py-3 space-y-3">
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
          <label className="flex items-center gap-2">
            <input
              type="checkbox"
              checked={config.send_by_enter}
              onChange={(e) => update("send_by_enter", e.target.checked)}
              className="rounded"
            />
            <span className="text-sm text-gray-600">Enter to send message</span>
          </label>
          <label className="flex items-center gap-2">
            <input
              type="checkbox"
              checked={config.rank_user_enable}
              onChange={(e) => update("rank_user_enable", e.target.checked)}
              className="rounded"
            />
            <span className="text-sm text-gray-600">Rank contacts by frequency</span>
          </label>
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

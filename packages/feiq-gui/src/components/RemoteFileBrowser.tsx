import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Folder, X, Loader2, Key } from "lucide-react";

/** A file entry from a remote directory listing */
export interface RemoteFileEntry {
  fileId: number;
  name: string;
  size: number;
  modifiedTime: number;
  isDir: boolean;
}

interface Props {
  /** IP of the peer whose shared folder to browse */
  peerIp: string;
  /** Peer display name */
  peerName: string;
  onClose: () => void;
}

export function RemoteFileBrowser({ peerIp, peerName, onClose }: Props) {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [password, setPassword] = useState("");
  const [showPassword, setShowPassword] = useState(false);
  const [requested, setRequested] = useState(false);

  const handleBrowse = async (pw?: string) => {
    setLoading(true);
    setError(null);
    try {
      await invoke("browse_shared_folder", {
        ip: peerIp,
        password: pw || null,
      });
      setRequested(true);
      // The response will arrive through the regular message pipeline.
      // For now, notify the user that the request was sent.
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40">
      <div className="bg-surface rounded-xl shadow-2xl w-[480px] max-h-[560px] flex flex-col border border-border">
        {/* Header */}
        <div className="px-4 py-3 border-b border-border flex items-center gap-3">
          <Folder className="w-5 h-5 text-primary" />
          <div className="flex-1 min-w-0">
            <div className="text-sm font-semibold text-text truncate">
              Browse: {peerName}
            </div>
            <div className="text-xs text-text-muted">{peerIp}</div>
          </div>
          <button
            onClick={onClose}
            className="w-7 h-7 flex items-center justify-center rounded-md hover:bg-surface-alt text-text-muted cursor-pointer"
            title="Close"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Body */}
        <div className="flex-1 px-4 py-3 overflow-y-auto">
          {error && (
            <div className="text-sm text-red-500 bg-red-50 dark:bg-red-900/20 rounded-md px-3 py-2 mb-3">
              {error}
            </div>
          )}

          {!requested && !loading && (
            <div className="text-center py-8">
              <Folder className="w-12 h-12 text-text-muted mx-auto mb-3" />
              <p className="text-sm text-text-muted mb-4">
                Send a directory listing request to{" "}
                <span className="font-medium text-text">{peerName}</span>
              </p>
              {showPassword ? (
                <div className="flex items-center gap-2 max-w-xs mx-auto">
                  <Key className="w-4 h-4 text-text-muted flex-shrink-0" />
                  <input
                    type="text"
                    value={password}
                    onChange={(e) => setPassword(e.target.value)}
                    placeholder="Shared folder password..."
                    className="flex-1 text-sm px-2 py-1.5 rounded-md border border-border
                               focus:outline-none focus:border-primary bg-surface"
                    onKeyDown={(e) => {
                      if (e.key === "Enter") handleBrowse(password || undefined);
                    }}
                  />
                </div>
              ) : (
                <div className="flex items-center justify-center gap-2">
                  <button
                    onClick={() => handleBrowse()}
                    className="px-4 py-2 text-sm font-medium bg-primary text-primary-foreground
                               rounded-md hover:opacity-90 cursor-pointer"
                  >
                    Browse Files
                  </button>
                  <button
                    onClick={() => setShowPassword(true)}
                    className="px-3 py-2 text-sm text-text-muted hover:text-text
                               rounded-md hover:bg-surface-alt cursor-pointer"
                    title="Browse with password"
                  >
                    <Key className="w-4 h-4" />
                  </button>
                </div>
              )}
            </div>
          )}

          {loading && (
            <div className="flex items-center justify-center py-12 gap-2 text-text-muted">
              <Loader2 className="w-5 h-5 animate-spin" />
              <span className="text-sm">Requesting directory listing...</span>
            </div>
          )}

          {requested && !loading && !error && (
            <div className="text-center py-8 text-text-muted text-sm">
              <p className="mb-2">Request sent!</p>
              <p>
                The directory listing will appear as file entries in the chat.
              </p>
              <p className="mt-1 text-xs">
                Click on individual files to download them.
              </p>
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="px-4 py-2 border-t border-border text-xs text-text-muted">
          Tip: Use the Settings dialog to configure your own shared folder and
          password.
        </div>
      </div>
    </div>
  );
}

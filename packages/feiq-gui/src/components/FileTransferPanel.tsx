import { useMemo } from "react";
import { useFileTransferStore, type FileTransfer } from "../stores/fileTransferStore";
import { invoke } from "@tauri-apps/api/core";
import { X, Download, Upload, Folder } from "lucide-react";

function isTerminal(state: string): boolean {
  return state === "finish" || state === "error" || state === "canceled";
}

/** Format file size in human-readable form */
function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes}B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)}KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)}MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)}GB`;
}

/** Folder transfer row with dual progress bars */
function FolderTransferRow({
  transfer,
  onCancel,
}: {
  transfer: FileTransfer;
  onCancel: (taskId: number) => void;
}) {
  const isTerminal =
    transfer.state === "finish" ||
    transfer.state === "error" ||
    transfer.state === "canceled";

  const overallPct =
    transfer.size > 0
      ? Math.min(100, (transfer.progress / transfer.size) * 100)
      : 0;

  const currentFilePct =
    transfer.currentFileTotal && transfer.currentFileTotal > 0
      ? Math.min(100, ((transfer.currentFileProgress || 0) / transfer.currentFileTotal) * 100)
      : 0;

  return (
    <div className="flex items-center gap-3 px-4 py-2 border-b border-border last:border-b-0">
      <div className="flex-shrink-0 w-6 h-6 flex items-center justify-center">
        {transfer.direction === "upload" ? (
          <Upload className="w-4 h-4 text-text-muted" />
        ) : (
          <Download className="w-4 h-4 text-text-muted" />
        )}
      </div>

      <div className="flex-1 min-w-0">
        <div className="flex items-center justify-between gap-2">
          <span className="text-sm text-text truncate flex items-center gap-1.5">
            <Folder className="w-3.5 h-3.5 text-amber-500 flex-shrink-0" />
            {transfer.folderName || transfer.filename}
          </span>
          <span className="text-xs text-text-muted flex-shrink-0">
            {transfer.filesCompleted || 0}/{transfer.totalFiles || 0} files
          </span>
        </div>

        {/* Overall progress bar */}
        {!isTerminal && (
          <div
            className="relative w-full h-1.5 bg-surface-alt rounded-full mt-1 overflow-hidden"
            role="progressbar"
            aria-valuenow={Math.round(overallPct)}
            aria-valuemin={0}
            aria-valuemax={100}
          >
            <div
              className="absolute inset-y-0 left-0 bg-primary rounded-full transition-all duration-300"
              style={{ width: `${overallPct}%` }}
            />
          </div>
        )}

        {/* Current file progress */}
        {!isTerminal && transfer.currentFile && (
          <div className="mt-1">
            <div className="flex items-center justify-between">
              <span className="text-xs text-text-muted truncate max-w-[70%]">
                {transfer.currentFile}
              </span>
              <span className="text-xs text-text-muted flex-shrink-0">
                {currentFilePct.toFixed(0)}%
              </span>
            </div>
            <div
              className="relative w-full h-1 bg-surface-alt/50 rounded-full mt-0.5 overflow-hidden"
            >
              <div
                className="absolute inset-y-0 left-0 bg-primary/60 rounded-full transition-all duration-300"
                style={{ width: `${currentFilePct}%` }}
              />
            </div>
          </div>
        )}

        {/* Status */}
        <div className="flex items-center justify-between mt-0.5">
          <span
            className={`text-xs ${
              transfer.state === "error"
                ? "text-red-500"
                : transfer.state === "finish"
                  ? "text-green-500"
                  : "text-text-muted"
            }`}
          >
            {statusText(transfer)}
          </span>
          {!isTerminal && (
            <button
              onClick={() => onCancel(transfer.taskId)}
              className="w-5 h-5 flex items-center justify-center rounded hover:bg-surface-alt text-text-muted hover:text-red-500 transition-colors cursor-pointer"
              title="Cancel transfer"
            >
              <X className="w-3.5 h-3.5" />
            </button>
          )}
        </div>
      </div>
    </div>
  );
}

/** Get human-readable status text for a transfer state */
function statusText(transfer: FileTransfer): string {
  switch (transfer.state) {
    case "not_start":
      return "Waiting...";
    case "running":
      return `${(transfer.progress / Math.max(transfer.size, 1) * 100).toFixed(1)}%`;
    case "finish":
      return "Completed";
    case "error":
      return `Error: ${transfer.errorMessage || "Unknown error"}`;
    case "canceled":
      return "Canceled";
  }
}

/** Single transfer row shown in the panel */
function TransferRow({
  transfer,
  onCancel,
}: {
  transfer: FileTransfer;
  onCancel: (taskId: number) => void;
}) {
  const pct =
    transfer.size > 0
      ? Math.min(100, (transfer.progress / transfer.size) * 100)
      : 0;
  const isTerminal =
    transfer.state === "finish" ||
    transfer.state === "error" ||
    transfer.state === "canceled";

  return (
    <div className="flex items-center gap-3 px-4 py-2 border-b border-border last:border-b-0">
      {/* Direction icon */}
      <div className="flex-shrink-0 w-6 h-6 flex items-center justify-center">
        {transfer.direction === "upload" ? (
          <Upload className="w-4 h-4 text-text-muted" />
        ) : (
          <Download className="w-4 h-4 text-text-muted" />
        )}
      </div>

      {/* File info and progress */}
      <div className="flex-1 min-w-0">
        <div className="flex items-center justify-between gap-2">
          <span className="text-sm text-text truncate">{transfer.filename}</span>
          <span className="text-xs text-text-muted flex-shrink-0">
            {formatSize(transfer.size)}
          </span>
        </div>

        {/* Progress bar (hidden for terminal states) */}
        {!isTerminal && (
          <div
            className="relative w-full h-1.5 bg-surface-alt rounded-full mt-1 overflow-hidden"
            role="progressbar"
            aria-valuenow={Math.round(pct)}
            aria-valuemin={0}
            aria-valuemax={100}
          >
            <div
              className="absolute inset-y-0 left-0 bg-primary rounded-full transition-all duration-300"
              style={{ width: `${pct}%` }}
            />
          </div>
        )}

        {/* Status text */}
        <div className="flex items-center justify-between mt-0.5">
          <span
            className={`text-xs ${
              transfer.state === "error"
                ? "text-red-500"
                : transfer.state === "finish"
                  ? "text-green-500"
                  : "text-text-muted"
            }`}
          >
            {statusText(transfer)}
          </span>

          {/* Cancel button (only for active transfers) */}
          {!isTerminal && (
            <button
              onClick={() => onCancel(transfer.taskId)}
              className="w-5 h-5 flex items-center justify-center rounded hover:bg-surface-alt text-text-muted hover:text-red-500 transition-colors cursor-pointer"
              title="Cancel transfer"
            >
              <X className="w-3.5 h-3.5" />
            </button>
          )}
        </div>
      </div>
    </div>
  );
}

export function FileTransferPanel() {
  const transfers = useFileTransferStore((s) => s.transfers);

  const allTransfers = Object.values(transfers);
  const active = useMemo(
    () => allTransfers.filter((t) => !isTerminal(t.state)),
    [transfers],
  );

  // Always show if there are any transfers at all (even completed ones briefly)
  if (allTransfers.length === 0) return null;

  const handleCancel = async (taskId: number) => {
    try {
      await invoke("cancel_file_task", { taskId });
    } catch (e) {
      console.error("cancel_file_task failed:", e);
    }
  };

  return (
    <div className="border-b border-border bg-surface max-h-48 overflow-y-auto">
      {active.length > 0 ? (
        active.map((t) =>
          t.isFolder ? (
            <FolderTransferRow key={t.taskId} transfer={t} onCancel={handleCancel} />
          ) : (
            <TransferRow key={t.taskId} transfer={t} onCancel={handleCancel} />
          ),
        )
      ) : (
        <div className="px-4 py-2 text-xs text-text-muted text-center">
          All file transfers completed
        </div>
      )}
    </div>
  );
}

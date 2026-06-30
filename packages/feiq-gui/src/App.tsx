import { useEffect, useState, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { Sidebar } from "./components/Sidebar";
import { ChatPanel } from "./components/ChatPanel";
import { useContactStore } from "./stores/contactStore";
import type { Fellow } from "./stores/contactStore";
import { useMessageStore } from "./stores/messageStore";
import type { Content } from "./stores/messageStore";
import { useFileTransferStore, type FileTransfer } from "./stores/fileTransferStore";
import { useGroupStore } from "./stores/groupStore";
import type { Group } from "./stores/groupStore";

export default function App() {
  const upsertContact = useContactStore((s) => s.upsertContact);
  const addMessage = useMessageStore((s) => s.addMessage);
  const setGroups = useGroupStore((s) => s.setGroups);
  const upsertTransfer = useFileTransferStore((s) => s.upsertTransfer);
  const selectedIp = useContactStore((s) => s.selectedIp);

  // ─── Drag-drop overlay state ───────────────────────────────
  const [dragOver, setDragOver] = useState(false);
  const dragCountRef = useRef(0);

  // ─── Load groups on startup ─────────────────────────────────
  useEffect(() => {
    invoke<[string, string[]][]>("get_groups")
      .then((rawGroups) => {
        const groups: Group[] = rawGroups.map(([name, memberIps]) => ({
          name,
          memberIps,
        }));
        setGroups(groups);
      })
      .catch((e) => console.error("Failed to load groups:", e));
  }, []);

  // ─── Tauri event listeners ─────────────────────────────────
  useEffect(() => {
    const unlisteners: (() => void)[] = [];

    listen<Fellow>("contact-update", (event) => {
      upsertContact(event.payload);
    }).then((fn) => unlisteners.push(fn));

    listen<{
      fromIp: string;
      fromName: string;
      contents: Content[];
      timestamp: number;
    }>("new-message", (event) => {
      const { fromIp, fromName, contents, timestamp } = event.payload;
      addMessage(fromIp, {
        fromIp,
        fromName,
        contents,
        timestamp,
        direction: "received",
      });
    });

    // File progress events
    listen<{ taskId: number; progress: number; total: number }>(
      "file-progress",
      (event) => {
        const { taskId, progress, total } = event.payload;
        upsertTransfer({
          taskId,
          filename: "",
          size: total,
          progress,
          state: "running",
          direction: "download",
        });
      },
    ).then((fn) => unlisteners.push(fn));

    // File state changed events
    listen<{ taskId: number; state: string }>(
      "file-state-changed",
      (event) => {
        const { taskId, state } = event.payload;
        upsertTransfer({
          taskId,
          filename: "",
          size: 0,
          progress: 0,
          state: state as FileTransfer["state"],
          direction: "download",
        });
      },
    ).then((fn) => unlisteners.push(fn));

    // Auto-start engine
    invoke("start_engine").catch((e) => console.error("Engine start failed:", e));

    return () => {
      unlisteners.forEach((fn) => fn());
    };
  }, []);

  // ─── Drag-drop event listener ──────────────────────────────
  useEffect(() => {
    let unlistenDrag: () => void;

    getCurrentWebview()
      .onDragDropEvent((event) => {
        const payload = event.payload;
        if (payload.type === "enter") {
          dragCountRef.current += 1;
          setDragOver(true);
        } else if (payload.type === "leave") {
          dragCountRef.current -= 1;
          if (dragCountRef.current <= 0) {
            dragCountRef.current = 0;
            setDragOver(false);
          }
        } else if (payload.type === "drop") {
          dragCountRef.current = 0;
          setDragOver(false);

          const targetIp = selectedIp;
          if (!targetIp || !payload.paths || payload.paths.length === 0) return;

          for (const filePath of payload.paths) {
            invoke("send_file", { ip: targetIp, filePath }).catch((e) =>
              console.error(`send_file failed for ${filePath}:`, e),
            );
          }
        }
      })
      .then((fn) => {
        unlistenDrag = fn;
      });

    return () => {
      if (unlistenDrag) unlistenDrag();
    };
  }, [selectedIp]);

  // ─── Apply theme class to document ─────────────────────────
  useEffect(() => {
    invoke("get_settings").then((config: any) => {
      const theme = config.theme || "auto";
      if (theme === "auto") {
        const mq = window.matchMedia("(prefers-color-scheme: dark)");
        document.documentElement.classList.toggle("theme-dark", mq.matches);
        mq.addEventListener("change", (e) => {
          document.documentElement.classList.toggle("theme-dark", e.matches);
        });
      } else if (theme === "dark") {
        document.documentElement.classList.add("theme-dark");
      } else {
        document.documentElement.classList.remove("theme-dark");
      }
    });
  }, []);

  // ─── Reset unread count on window focus ────────────────────
  useEffect(() => {
    let unlistenFocus: () => void;
    listen("tauri://focus", () => {
      invoke("reset_unread_count").catch(() => {});
    }).then((fn) => {
      unlistenFocus = fn;
    });
    return () => {
      if (unlistenFocus) unlistenFocus();
    };
  }, []);

  return (
    <div className="flex h-screen w-screen bg-bg relative">
      <Sidebar />
      <ChatPanel />

      {/* Drag-drop overlay */}
      {dragOver && (
        <div className="absolute inset-0 z-50 pointer-events-none flex items-center justify-center">
          <div className="absolute inset-0 bg-primary/10" />
          <div className="relative border-2 border-dashed border-primary/50 rounded-2xl px-12 py-8 bg-surface/80 backdrop-blur-sm">
            <p className="text-lg font-semibold text-primary">Drop files to send</p>
            <p className="text-sm text-text-muted mt-1 text-center">
              Files will be sent to the selected contact
            </p>
          </div>
        </div>
      )}
    </div>
  );
}

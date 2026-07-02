import { useState, useRef, useEffect, type KeyboardEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Send, Smile, Paperclip, FolderOpen } from "lucide-react";
import { useMessageStore } from "../stores/messageStore";
import { useContactStore } from "../stores/contactStore";
import { EmojiPicker } from "./EmojiPicker";
import { open as dialogOpen } from "@tauri-apps/plugin-dialog";

export function InputArea({ fellowIp }: { fellowIp: string }) {
  const [text, setText] = useState("");
  const [showEmoji, setShowEmoji] = useState(false);
  const [sendByEnter, setSendByEnter] = useState(true);
  const inputRef = useRef<HTMLTextAreaElement>(null);

  // Load send_by_enter preference from backend
  useEffect(() => {
    invoke<{ send_by_enter?: boolean }>("get_settings")
      .then((s) => setSendByEnter(s.send_by_enter ?? true))
      .catch(() => {});
  }, []);

  const addMessage = useMessageStore((s) => s.addMessage);
  const contacts = useContactStore((s) => s.contacts);

  const sendText = async () => {
    const trimmed = text.trim();
    if (!trimmed) return;

    const fellow = contacts.find((c) => c.ip === fellowIp);

    addMessage(fellowIp, {
      fromIp: "self",
      fromName: "Me",
      contents: [{ type: "text", text: trimmed }],
      timestamp: Date.now(),
      direction: "sent",
    });

    setText("");
    inputRef.current?.focus();

    if (fellow) {
      invoke("send_text", { ip: fellow.ip, text: trimmed })
        .catch((e) => console.error("Send failed:", e));
    }
  };

  const handleKeyDown = (e: KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      if (sendByEnter) {
        e.preventDefault();
        sendText();
      }
    }
  };

  const insertEmoji = (code: string) => {
    setText((prev) => prev + code);
    setShowEmoji(false);
    inputRef.current?.focus();
  };

  const handleSendFiles = async () => {
    try {
      const selected = await dialogOpen({ multiple: true });
      if (!selected) return;

      const paths = Array.isArray(selected) ? selected : [selected];
      const fellow = contacts.find((c) => c.ip === fellowIp);
      if (!fellow) { console.warn("No contact selected"); return; }

      for (const filePath of paths) {
        if (!filePath) continue;
        try {
          await invoke("send_file", { ip: fellow.ip, filePath });
        } catch (e) {
          console.error(`send_file failed for ${filePath}:`, e);
        }
      }
    } catch (e) {
      console.error("File dialog failed:", e);
    }
  };

  const handleSendFolder = async () => {
    try {
      const selected = await dialogOpen({ directory: true, multiple: false });
      if (!selected) return;

      // Normalize: on some platforms the dialog may return an array
      const folderPath = Array.isArray(selected) ? selected[0] : selected;
      if (!folderPath) return;

      const fellow = contacts.find((c) => c.ip === fellowIp);
      if (!fellow) {
        console.warn("No contact selected for folder transfer");
        return;
      }

      try {
        // Use "send_file" IPC — the backend auto-detects directories
        // via std::fs::metadata and routes to send_folder_to accordingly.
        await invoke("send_file", { ip: fellow.ip, filePath: folderPath });
      } catch (e) {
        console.error("send_folder failed:", e);
        alert(`Failed to send folder: ${e}`);
      }
    } catch (e) {
      console.error("Folder dialog failed:", e);
      alert(`Folder dialog error: ${e}`);
    }
  };

  return (
    <div className="border-t border-border px-4 py-3 bg-surface-alt relative">
      {showEmoji && (
        <EmojiPicker onSelect={insertEmoji} onClose={() => setShowEmoji(false)} />
      )}

      <div className="flex items-end gap-2">
        {/* Send files button */}
        <button
          onClick={handleSendFiles}
          className="flex-shrink-0 w-8 h-8 flex items-center justify-center
                     rounded-lg hover:bg-surface-alt text-text-muted transition-colors cursor-pointer mb-1"
          title="Send file(s)"
        >
          <Paperclip className="w-5 h-5" />
        </button>

        {/* Send folder button */}
        <button
          onClick={handleSendFolder}
          className="flex-shrink-0 w-8 h-8 flex items-center justify-center
                     rounded-lg hover:bg-surface-alt text-text-muted transition-colors cursor-pointer mb-1"
          title="Send folder"
        >
          <FolderOpen className="w-5 h-5" />
        </button>

        {/* Emoji button */}
        <button
          onClick={() => setShowEmoji(!showEmoji)}
          className="flex-shrink-0 w-8 h-8 flex items-center justify-center
                     rounded-lg hover:bg-surface-alt text-text-muted transition-colors cursor-pointer mb-1"
        >
          <Smile className="w-5 h-5" />
        </button>

        <textarea
          ref={inputRef}
          value={text}
          onChange={(e) => setText(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="Type a message... (Enter to send)"
          rows={2}
          className="flex-1 resize-none text-sm px-3 py-2 rounded-lg border border-border
                     focus:outline-none focus:border-primary bg-surface"
        />
        <button
          onClick={sendText}
          disabled={!text.trim()}
          className="flex-shrink-0 w-10 h-10 flex items-center justify-center
                     rounded-lg bg-primary text-white hover:bg-primary-hover
                     disabled:bg-offline transition-colors cursor-pointer"
        >
          <Send className="w-4 h-4" />
        </button>
      </div>
    </div>
  );
}

import { useState, useRef, type KeyboardEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Send, Smile, Camera } from "lucide-react";
import { useMessageStore } from "../stores/messageStore";
import { useContactStore } from "../stores/contactStore";
import { EmojiPicker } from "./EmojiPicker";
import { open as dialogOpen } from "@tauri-apps/plugin-dialog";

export function InputArea({ fellowIp }: { fellowIp: string }) {
  const [text, setText] = useState("");
  const [showEmoji, setShowEmoji] = useState(false);
  const inputRef = useRef<HTMLTextAreaElement>(null);
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

    // Actually send over network
    if (fellow) {
      invoke("send_text", { ip: fellow.ip, text: trimmed })
        .catch((e) => console.error("Send failed:", e));
    }
  };

  const handleKeyDown = (e: KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      sendText();
    }
  };

  const insertEmoji = (code: string) => {
    setText((prev) => prev + code);
    setShowEmoji(false);
    inputRef.current?.focus();
  };

  const handleScreenshot = async () => {
    try {
      const result = await invoke<string>("capture_screenshot");
      if (result === "FALLBACK") {
        // User canceled or platform not supported — open file dialog
        const filePath = await dialogOpen({
          multiple: false,
          filters: [
            {
              name: "Images",
              extensions: ["png", "jpg", "jpeg", "gif", "bmp"],
            },
            { name: "All Files", extensions: ["*"] },
          ],
        });
        if (!filePath) return;
        const path = Array.isArray(filePath) ? filePath[0] : filePath;
        // Show the file path in a message (actual file sending to come later)
        const fellow = contacts.find((c) => c.ip === fellowIp);
        if (fellow) {
          addMessage(fellowIp, {
            fromIp: "self",
            fromName: "Me",
            contents: [{ type: "text", text: `[Screenshot/File] ${path}` }],
            timestamp: Date.now(),
            direction: "sent",
          });
        }
      } else {
        // Screenshot captured — show file path
        const fellow = contacts.find((c) => c.ip === fellowIp);
        if (fellow) {
          addMessage(fellowIp, {
            fromIp: "self",
            fromName: "Me",
            contents: [{ type: "text", text: `[Screenshot] ${result}` }],
            timestamp: Date.now(),
            direction: "sent",
          });
        }
      }
    } catch (e) {
      console.error("Screenshot failed:", e);
    }
  };

  return (
    <div className="border-t border-border px-4 py-3 bg-surface-alt relative">
      {/* Emoji Picker */}
      {showEmoji && (
        <EmojiPicker onSelect={insertEmoji} onClose={() => setShowEmoji(false)} />
      )}

      <div className="flex items-end gap-2">
        {/* Screenshot button */}
        <button
          onClick={handleScreenshot}
          className="flex-shrink-0 w-8 h-8 flex items-center justify-center
                     rounded-lg hover:bg-surface-alt text-text-muted transition-colors cursor-pointer mb-1"
          title="Capture screenshot"
        >
          <Camera className="w-5 h-5" />
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

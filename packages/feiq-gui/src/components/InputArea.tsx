import { useState, useRef, useEffect, type KeyboardEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Send, Smile, Paperclip, PenLine } from "lucide-react";
import { useMessageStore } from "../stores/messageStore";
import { useContactStore } from "../stores/contactStore";
import { EmojiPicker } from "./EmojiPicker";
import { DoodleDialog } from "./DoodleDialog";
import { open as dialogOpen } from "@tauri-apps/plugin-dialog";

export function InputArea({ fellowIp }: { fellowIp: string }) {
  const [text, setText] = useState("");
  const [showEmoji, setShowEmoji] = useState(false);
  const [showDoodle, setShowDoodle] = useState(false);
  const [sendByEnter, setSendByEnter] = useState(true);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const typingRef = useRef(false);
  const typingTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Load send_by_enter preference from backend
  useEffect(() => {
    invoke<{ send_by_enter?: boolean }>("get_settings")
      .then((s) => setSendByEnter(s.send_by_enter ?? true))
      .catch(() => {});
  }, []);

  const addMessage = useMessageStore((s) => s.addMessage);
  const contacts = useContactStore((s) => s.contacts);
  const fellow = contacts.find((c) => c.ip === fellowIp);

  // Send typing indicator with debounce
  const notifyTyping = (isTyping: boolean) => {
    invoke("send_typing", { ip: fellowIp, isTyping }).catch(() => {});
  };

  const handleTypingChange = (value: string) => {
    setText(value);

    if (value.length > 0 && !typingRef.current) {
      // Started typing
      typingRef.current = true;
      notifyTyping(true);
    }

    // Reset the idle timeout
    if (typingTimeoutRef.current) {
      clearTimeout(typingTimeoutRef.current);
    }

    if (value.length > 0) {
      // After 3s of no change, consider typing stopped
      typingTimeoutRef.current = setTimeout(() => {
        if (typingRef.current) {
          typingRef.current = false;
          notifyTyping(false);
        }
      }, 3000);
    } else {
      // Text cleared: stop typing immediately
      if (typingRef.current) {
        typingRef.current = false;
        notifyTyping(false);
      }
    }
  };

  // Cleanup typing state on unmount or fellowIp change
  useEffect(() => {
    return () => {
      if (typingRef.current) {
        notifyTyping(false);
        typingRef.current = false;
      }
      if (typingTimeoutRef.current) {
        clearTimeout(typingTimeoutRef.current);
      }
    };
  }, [fellowIp]);

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

    // Clear typing state on send
    if (typingRef.current) {
      typingRef.current = false;
      notifyTyping(false);
    }
    if (typingTimeoutRef.current) {
      clearTimeout(typingTimeoutRef.current);
    }

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

  return (
    <>
    {showDoodle && fellow && (
      <DoodleDialog peerIp={fellowIp} onClose={() => setShowDoodle(false)} />
    )}
    <div className="border-t border-border px-4 py-3 bg-surface-alt relative">
      {showEmoji && (
        <EmojiPicker onSelect={insertEmoji} onClose={() => setShowEmoji(false)} />
      )}

      <div className="flex items-end gap-2">
        {/* Doodle button */}
        <button
          onClick={() => setShowDoodle(true)}
          className="flex-shrink-0 w-8 h-8 flex items-center justify-center
                     rounded-lg hover:bg-surface-alt text-text-muted transition-colors cursor-pointer mb-1"
          title="Draw a doodle"
        >
          <PenLine className="w-5 h-5" />
        </button>

        {/* Send files button */}
        <button
          onClick={handleSendFiles}
          className="flex-shrink-0 w-8 h-8 flex items-center justify-center
                     rounded-lg hover:bg-surface-alt text-text-muted transition-colors cursor-pointer mb-1"
          title="Send file(s)"
        >
          <Paperclip className="w-5 h-5" />
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
          onChange={(e) => handleTypingChange(e.target.value)}
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
    </>
  );
}

import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { Sidebar } from "./components/Sidebar";
import { ChatPanel } from "./components/ChatPanel";
import { useContactStore, Fellow } from "./stores/contactStore";
import { useMessageStore, Message, Content } from "./stores/messageStore";

export default function App() {
  const upsertContact = useContactStore((s) => s.upsertContact);
  const addMessage = useMessageStore((s) => s.addMessage);

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

    // Auto-start engine
    invoke("start_engine").catch((e) => console.error("Engine start failed:", e));

    return () => {
      unlisteners.forEach((fn) => fn());
    };
  }, []);

  return (
    <div className="flex h-screen w-screen bg-gray-100">
      <Sidebar />
      <ChatPanel />
    </div>
  );
}

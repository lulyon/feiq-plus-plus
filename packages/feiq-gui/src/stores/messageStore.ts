import { create } from "zustand";

export interface Content {
  type: "text" | "knock" | "file" | "image" | "id";
  text?: string;
  format?: string;
  filename?: string;
  size?: number;
}

export interface Message {
  fromIp: string;
  fromName: string;
  contents: Content[];
  timestamp: number;
  direction: "sent" | "received";
}

interface MessageStore {
  messagesByIp: Record<string, Message[]>;
  unreadByIp: Record<string, number>;
  addMessage: (ip: string, msg: Message) => void;
  markRead: (ip: string) => void;
}

export const useMessageStore = create<MessageStore>((set) => ({
  messagesByIp: {},
  unreadByIp: {},
  addMessage: (ip, msg) =>
    set((state) => {
      const messages = [...(state.messagesByIp[ip] || []), msg];
      const isSelected = useContactStore.getState().selectedIp === ip;
      const unreadByIp = { ...state.unreadByIp };
      if (!isSelected) {
        unreadByIp[ip] = (unreadByIp[ip] || 0) + 1;
      }
      return {
        messagesByIp: { ...state.messagesByIp, [ip]: messages },
        unreadByIp,
      };
    }),
  markRead: (ip) =>
    set((state) => ({
      unreadByIp: { ...state.unreadByIp, [ip]: 0 },
    })),
}));

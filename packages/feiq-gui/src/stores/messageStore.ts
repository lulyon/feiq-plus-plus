import { create } from "zustand";
import { useContactStore } from "./contactStore";

// Content can be either internally-tagged (frontend) or externally-tagged (Rust serde)
// The MessageBubble normalizes both formats, so this type is intentionally loose.
export interface Content {
  type?: string;
  text?: string | Record<string, unknown>;
  format?: string;
  filename?: string;
  size?: number;
  /** Local task ID assigned by engine for file transfers */
  localTaskId?: number;
  knock?: unknown;
  file?: Record<string, unknown>;
  image?: Record<string, unknown>;
  id?: Record<string, unknown>;
}

export interface Message {
  /** Database row id from backend history (undefined for live incoming messages) */
  id?: number;
  fromIp: string;
  fromName: string;
  contents: Content[];
  timestamp: number;
  direction: "sent" | "received";
}

interface MessageStore {
  messagesByIp: Record<string, Message[]>;
  unreadByIp: Record<string, number>;
  hasHistory: Record<string, boolean>;
  loadingHistory: Record<string, boolean>;
  historyOffset: Record<string, number>;
  addMessage: (ip: string, msg: Message) => void;
  prependMessages: (ip: string, msgs: Message[]) => void;
  markRead: (ip: string) => void;
  setHasHistory: (ip: string, has: boolean) => void;
  setLoadingHistory: (ip: string, loading: boolean) => void;
  setHistoryOffset: (ip: string, offset: number) => void;
}

export const useMessageStore = create<MessageStore>((set) => ({
  messagesByIp: {},
  unreadByIp: {},
  hasHistory: {},
  loadingHistory: {},
  historyOffset: {},
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
  prependMessages: (ip, msgs) =>
    set((state) => {
      const existing = state.messagesByIp[ip] || [];
      // Avoid duplicates: prefer id-based dedup (from history), fallback to timestamp
      const existingIds = new Set(
        existing.map((m) => m.id).filter((id): id is number => id != undefined),
      );
      const existingTimestamps = new Set(existing.map((m) => m.timestamp));
      const newMsgs = msgs.filter((m) => {
        if (m.id != null) return !existingIds.has(m.id);
        return !existingTimestamps.has(m.timestamp);
      });
      if (newMsgs.length === 0) return state;
      return {
        messagesByIp: {
          ...state.messagesByIp,
          [ip]: [...newMsgs, ...existing],
        },
      };
    }),
  markRead: (ip) =>
    set((state) => ({
      unreadByIp: { ...state.unreadByIp, [ip]: 0 },
    })),
  setHasHistory: (ip, has) =>
    set((state) => ({
      hasHistory: { ...state.hasHistory, [ip]: has },
    })),
  setLoadingHistory: (ip, loading) =>
    set((state) => ({
      loadingHistory: { ...state.loadingHistory, [ip]: loading },
    })),
  setHistoryOffset: (ip, offset) =>
    set((state) => ({
      historyOffset: { ...state.historyOffset, [ip]: offset },
    })),
}));

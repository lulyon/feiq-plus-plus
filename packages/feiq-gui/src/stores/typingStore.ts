import { create } from "zustand";

interface TypingState {
  /** Map of IP -> { name, expiresAt } */
  typers: Record<string, { name: string; expiresAt: number }>;
  setTyping: (ip: string, name: string) => void;
  clearTyping: (ip: string) => void;
  /** Clean expired typing indicators (call periodically) */
  cleanup: () => void;
}

const TYPING_TIMEOUT_MS = 5000;

export const useTypingStore = create<TypingState>((set, _get) => ({
  typers: {},
  setTyping: (ip, name) => {
    set((state) => ({
      typers: {
        ...state.typers,
        [ip]: { name, expiresAt: Date.now() + TYPING_TIMEOUT_MS },
      },
    }));
  },
  clearTyping: (ip) => {
    set((state) => {
      const { [ip]: _, ...rest } = state.typers;
      return { typers: rest };
    });
  },
  cleanup: () => {
    const now = Date.now();
    set((state) => {
      const updated: Record<string, { name: string; expiresAt: number }> = {};
      let changed = false;
      for (const [ip, info] of Object.entries(state.typers)) {
        if (info.expiresAt > now) {
          updated[ip] = info;
        } else {
          changed = true;
        }
      }
      return changed ? { typers: updated } : state;
    });
  },
}));

// Auto-cleanup every 2 seconds
setInterval(() => {
  useTypingStore.getState().cleanup();
}, 2000);

import { create } from "zustand";

export interface Fellow {
  ip: string;
  pc_name: string;
  name: string;
  host: string;
  mac: string;
  online: boolean;
  version: string;
  alias: string;
  group_name: string;
  signature: string;
  port: number;
  source?: "LanPeer" | { RelayPeer: string };
}

interface ContactStore {
  contacts: Fellow[];
  selectedIp: string | null;
  setContacts: (contacts: Fellow[]) => void;
  upsertContact: (fellow: Fellow) => void;
  selectContact: (ip: string | null) => void;
}

export const useContactStore = create<ContactStore>((set) => ({
  contacts: [],
  selectedIp: null,
  setContacts: (contacts) => set({ contacts }),
  upsertContact: (fellow) =>
    set((state) => {
      const idx = state.contacts.findIndex((c) => c.ip === fellow.ip);
      const contacts = [...state.contacts];
      if (idx >= 0) {
        // Preserve user-set alias/signature/group_name from network overwrites
        contacts[idx] = {
          ...contacts[idx],
          ...fellow,
          alias: contacts[idx].alias || fellow.alias,
          signature: contacts[idx].signature || fellow.signature,
          group_name: contacts[idx].group_name || fellow.group_name,
        };
      } else {
        contacts.push(fellow);
      }
      contacts.sort((a, b) => {
        if (a.online !== b.online) return a.online ? -1 : 1;
        return (a.alias || a.name || a.ip).localeCompare(b.alias || b.name || b.ip);
      });
      return { contacts };
    }),
  selectContact: (ip) => set({ selectedIp: ip }),
}));

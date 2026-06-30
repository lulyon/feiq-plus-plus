import { create } from "zustand";

export interface Group {
  name: string;
  memberIps: string[];
}

interface GroupStore {
  groups: Group[];
  selectedGroupName: string | null;
  setGroups: (groups: Group[]) => void;
  selectGroup: (name: string | null) => void;
  addGroup: (group: Group) => void;
}

export const useGroupStore = create<GroupStore>((set) => ({
  groups: [],
  selectedGroupName: null,
  setGroups: (groups) => set({ groups }),
  selectGroup: (name) => set({ selectedGroupName: name }),
  addGroup: (group) =>
    set((state) => ({
      groups: [...state.groups, group],
    })),
}));

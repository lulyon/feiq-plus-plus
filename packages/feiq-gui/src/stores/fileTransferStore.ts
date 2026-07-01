import { create } from "zustand";

export interface FileTransfer {
  taskId: number;
  filename: string;
  size: number;
  progress: number;
  state: "not_start" | "running" | "finish" | "error" | "canceled";
  direction: "upload" | "download";
  errorMessage?: string;
  /** Folder transfer specific fields */
  isFolder?: boolean;
  folderName?: string;
  filesCompleted?: number;
  totalFiles?: number;
  currentFile?: string;
  currentFileProgress?: number;
  currentFileTotal?: number;
}

interface FileTransferStore {
  transfers: Record<number, FileTransfer>;
  upsertTransfer: (transfer: FileTransfer) => void;
  removeTransfer: (taskId: number) => void;
  /** Transfers that are not finished, errored, or canceled */
  activeTransfers: () => FileTransfer[];
}

export const useFileTransferStore = create<FileTransferStore>((set, get) => ({
  transfers: {},

  upsertTransfer: (transfer) =>
    set((state) => ({
      transfers: {
        ...state.transfers,
        [transfer.taskId]: transfer,
      },
    })),

  removeTransfer: (taskId) =>
    set((state) => {
      const transfers = { ...state.transfers };
      delete transfers[taskId];
      return { transfers };
    }),

  activeTransfers: () => {
    const transfers = Object.values(get().transfers);
    return transfers.filter(
      (t) =>
        t.state !== "finish" &&
        t.state !== "error" &&
        t.state !== "canceled",
    );
  },
}));

import { create } from "zustand";
import type { DbFile } from "@/lib/db";

/**
 * The Notion-style file peek: any list row in a subject opens its file in a
 * right-hand slide-over instead of navigating away. One peek at a time,
 * app-wide; it closes itself when the route leaves the file's subject.
 */
interface PeekState {
  file: DbFile | null;
  openFile: (file: DbFile) => void;
  close: () => void;
}

export const usePeekStore = create<PeekState>((set) => ({
  file: null,
  openFile: (file) => set({ file }),
  close: () => set({ file: null }),
}));

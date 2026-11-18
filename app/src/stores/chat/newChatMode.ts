import { create } from "zustand";

import type { PermissionMode } from "@/lib/harness";

/**
 * How the next new chat asks: the permission mode picked in a composer that
 * has no thread yet (`usePermissionMode`). Every send that creates a thread
 * from a composer carries it as `startMode`, and Rust writes it to the row
 * with the thread. The session's, like its model pick: it stays for the next
 * new chat and is not saved across launches.
 */
export const useNewChatMode = create<{ mode: PermissionMode; set: (mode: PermissionMode) => void }>((set) => ({
  mode: "ask",
  set: (mode) => set({ mode }),
}));

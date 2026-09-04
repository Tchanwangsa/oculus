import { create } from "zustand";
import {
  archiveInboxItem,
  getInboxItems,
  getUnreadInboxCount,
  markInboxRead,
  type DbInboxItem,
} from "@/lib/db";

interface InboxState {
  items: DbInboxItem[];
  unread: number;
  loaded: boolean;

  /** Re-read from the database. The digest calls this as summaries land. */
  refresh: (includeArchived?: boolean) => Promise<void>;
  markRead: (id: number) => Promise<void>;
  archive: (id: number) => Promise<void>;
}

export const useInboxStore = create<InboxState>((set, get) => ({
  items: [],
  unread: 0,
  loaded: false,

  refresh: async (includeArchived = false) => {
    const [items, unread] = await Promise.all([
      getInboxItems(includeArchived),
      getUnreadInboxCount(),
    ]);
    set({ items, unread, loaded: true });
  },

  markRead: async (id) => {
    await markInboxRead(id);
    await get().refresh();
  },

  archive: async (id) => {
    await archiveInboxItem(id);
    await get().refresh();
  },
}));

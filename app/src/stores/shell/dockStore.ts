import { create } from "zustand";
import { shallow } from "zustand/shallow";
import { dock, type DockPageInfo, type DockSnapshot, type InboxItem } from "@/lib/dock";

/** The Inbox keeps this many arrivals; older ones fall off the end. */
const INBOX_CAP = 200;

/** A mirror of Rust's dock pages and Inbox (`app/src-tauri/src/shell/dock/`),
 *  fed by `useDockPages`. Separate from `browserStore` on purpose: a dock page
 *  in the browser's list would be adopted as a browser tab. The Inbox lives
 *  here and in Rust's memory only, never in storage. */
interface DockState {
  pages: DockPageInfo[];
  shown: string | null;
  /** Newest first. */
  inbox: InboxItem[];
  apply: (snapshot: DockSnapshot) => void;
  /** Folds in Rust's list at boot, keeping any arrival the event already
   *  brought while the list was in flight. */
  loadInbox: (items: InboxItem[]) => void;
  /** Prepends one arrival from `dock-inbox`, skipping an id already held. */
  receive: (item: InboxItem) => void;
  clearInbox: () => Promise<void>;
}

export const useDockStore = create<DockState>((set) => ({
  pages: [],
  shown: null,
  inbox: [],
  apply: (snapshot) =>
    set((s) => {
      const previous = new Map(s.pages.map((page) => [page.id, page]));
      // Keep unchanged page objects, so other services' selectors stay quiet.
      const pages = snapshot.pages.map((page) => {
        const cached = previous.get(page.id);
        return cached && shallow(cached, page) ? cached : page;
      });
      return shallow(s.pages, pages) && s.shown === snapshot.shown
        ? s
        : { pages, shown: snapshot.shown };
    }),
  loadInbox: (items) =>
    set((s) => {
      const held = new Set(s.inbox.map((i) => i.id));
      const inbox = [...s.inbox, ...items.filter((i) => !held.has(i.id))];
      return { inbox: inbox.sort((a, b) => b.at - a.at).slice(0, INBOX_CAP) };
    }),
  receive: (item) =>
    set((s) =>
      s.inbox.some((i) => i.id === item.id)
        ? s
        : { inbox: [item, ...s.inbox].slice(0, INBOX_CAP) },
    ),
  clearInbox: async () => {
    await dock.clearInbox();
    set({ inbox: [] });
  },
}));

/** Unread across the enabled services, for the Messaging rail item. */
export function totalUnread(pages: DockPageInfo[], enabled: readonly string[]): number {
  return pages.reduce((sum, p) => (enabled.includes(p.id) ? sum + (p.unread || 0) : sum), 0);
}

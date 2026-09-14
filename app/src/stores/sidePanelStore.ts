import { create } from "zustand";
import type { DbFile, Lecture } from "@/lib/db";
import { useTabStore } from "@/stores/tabStore";

/** What the side panel is showing. One item at a time, per tab. */
export type PanelItem =
  | { kind: "file"; file: DbFile }
  | { kind: "lecture"; lecture: Lecture };

/** Identity within a tab — what a re-open compares against, and the React key
 *  that remounts the body when the panel swaps to a different thing. */
export function itemKey(item: PanelItem): string {
  return item.kind === "file"
    ? `file:${item.file.relative_path}`
    : `lecture:${item.lecture.id}`;
}

interface SidePanelState {
  /** Open item per tab id. A tab with no entry has the panel shut. */
  items: Record<number, PanelItem | undefined>;
  /** Opens into the tab in front — see the note on `open` below. */
  open: (item: PanelItem) => void;
  close: (tabId: number) => void;
  /** Refresh the open item in place, if it is still the same one. */
  sync: (tabId: number, item: PanelItem) => void;
  /** Called when a tab goes away, so its item does too. */
  dropTab: (tabId: number) => void;
}

/**
 * The side panel's contents, one entry per tab.
 *
 * It used to be a single app-wide file (`peekStore`) rendered inside whichever
 * pane was in front, which meant two tabs shared one open file and the panel
 * appeared to jump between them. The panel is drawn once, as furniture inside
 * the content card, and reads the active tab's entry — so each tab keeps what
 * it opened and switching tabs swaps the contents rather than moving a panel.
 *
 * The panel's *size* is deliberately not here: width and collapsed live in the
 * one `useResizablePanel` the panel component owns, so dragging it wider in one
 * tab widens it everywhere, the way a window's furniture should behave.
 */
export const useSidePanelStore = create<SidePanelState>((set) => ({
  items: {},

  /**
   * Takes no tab id, and doesn't need one: the panes that aren't in front are
   * `inert` (see `TabPane`), so a click can only ever originate in the active
   * tab. That is what keeps every list row in the app calling `openFileSmart`
   * with nothing but a file.
   */
  open: (item) => {
    const tabId = useTabStore.getState().activeId;
    if (tabId == null) return;
    set((s) => ({ items: { ...s.items, [tabId]: item } }));
  },

  /**
   * For a list that has re-fetched the row it has open — lecture progress
   * ticking over, say. Unlike `open` this names its tab, because the caller
   * can be a page in a tab that is not in front, and it is a no-op unless that
   * tab still has the same item open, so a stale refresh cannot reopen
   * something the user just closed.
   */
  sync: (tabId, item) =>
    set((s) => {
      const cur = s.items[tabId];
      if (!cur || itemKey(cur) !== itemKey(item)) return s;
      return { items: { ...s.items, [tabId]: item } };
    }),

  close: (tabId) =>
    set((s) => {
      if (!s.items[tabId]) return s;
      const next = { ...s.items };
      delete next[tabId];
      return { items: next };
    }),

  dropTab: (tabId) =>
    set((s) => {
      if (!(tabId in s.items)) return s;
      const next = { ...s.items };
      delete next[tabId];
      return { items: next };
    }),
}));

/** The item the panel should be showing: the active tab's, or nothing. */
export function useActivePanelItem(): PanelItem | null {
  const activeId = useTabStore((s) => s.activeId);
  return useSidePanelStore((s) => (activeId == null ? null : s.items[activeId] ?? null));
}

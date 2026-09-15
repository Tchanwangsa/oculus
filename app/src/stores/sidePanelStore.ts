import { create } from "zustand";
import type { DbFile, Lecture } from "@/lib/db";
import { ownsPlayback, stopLecturePlayback } from "@/lib/lecturePlayback";
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
  /**
   * How many times anything has been opened. The panel unfolds on this rather
   * than on the item changing, because re-opening the row that is *already*
   * showing leaves the item identical — and a folded panel has to unfold for
   * that click too, or clicking the file you last looked at does nothing at
   * all. Folding is the panel's own state (`useResizablePanel`), out of reach
   * from here, so a counter is how a click reaches it.
   */
  opens: number;
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
  opens: 0,

  /**
   * Takes no tab id, and doesn't need one: the panes that aren't in front are
   * `inert` (see `TabPane`), so a click can only ever originate in the active
   * tab. That is what keeps every list row in the app calling `openFileSmart`
   * with nothing but a file.
   */
  open: (item) => {
    const tabId = useTabStore.getState().activeId;
    if (tabId == null) return;
    set((s) => ({ items: { ...s.items, [tabId]: item }, opens: s.opens + 1 }));
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

/**
 * Shuts whatever the tab in front has open — what the shell does on its way
 * out of a page (`navigateActive` in `app/src/lib/tabRouters.ts`).
 *
 * The panel is furniture *beside* a page, and what is in it was opened from
 * that page: a lecture from the Lectures tab, a file from Downloads. Clicking
 * Calendar in the sidebar is leaving that page, so carrying the peek across
 * would dock half the card to something the new page has no relation to.
 * `FilePanel` already enforced the subject half of this rule from inside
 * itself; here it is the one rule, at the one door the shell navigates
 * through, so it holds for lectures and for a move within the same subject
 * too.
 *
 * A lecture peek is a player, so closing it is a stop — the pairing
 * `LecturePanel`'s × makes. The position is written on the way out, so the
 * lecture resumes where it was; leaving it playing under a page that no longer
 * shows it is the worse of the two.
 */
export function closeActivePanel(): void {
  const tabId = useTabStore.getState().activeId;
  const { items, close } = useSidePanelStore.getState();
  const item = items[tabId];
  if (!item) return;
  if (item.kind === "lecture" && ownsPlayback(tabId)) stopLecturePlayback();
  close(tabId);
}

/** The item the panel should be showing: the active tab's, or nothing. */
export function useActivePanelItem(): PanelItem | null {
  const activeId = useTabStore((s) => s.activeId);
  return useSidePanelStore((s) => (activeId == null ? null : s.items[activeId] ?? null));
}

import { create } from "zustand";
import type { DbFile, Lecture } from "@/lib/db";
import { ownsPlayback, stopLecturePlayback } from "@/lib/lecturePlayback";
import { activePane, useTabStore, focusedPane } from "@/stores/tabStore";

/** What the side panel is showing. One item at a time, per pane — a split
 *  tab's two halves each peek at their own thing. */
export type PanelItem =
  | { kind: "file"; file: DbFile }
  | { kind: "lecture"; lecture: Lecture };

/** Identity within a pane — what a re-open compares against, and the React key
 *  that remounts the body when the panel swaps to a different thing. */
export function itemKey(item: PanelItem): string {
  return item.kind === "file"
    ? `file:${item.file.relative_path}`
    : `lecture:${item.lecture.id}`;
}

interface SidePanelState {
  /** Open item per pane id. A pane with no entry has the panel shut. */
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
  close: (paneId: number) => void;
  /** Refresh the open item in place, if it is still the same one. */
  sync: (paneId: number, item: PanelItem) => void;
  /** Called when a pane goes away — a tab closed, a split folded — so its
   *  item does too. */
  dropTab: (paneId: number) => void;
}

/**
 * The side panel's contents, one entry per pane.
 *
 * It used to be a single app-wide file (`peekStore`) rendered inside whichever
 * pane was in front, which meant two tabs shared one open file and the panel
 * appeared to jump between them. The panel is drawn once, as furniture inside
 * the content card, and reads the focused pane's entry — so each pane keeps
 * what it opened and switching tabs, or halves, swaps the contents rather than
 * moving a panel.
 *
 * The panel's *size* is deliberately not here: width and collapsed live in the
 * one `useResizablePanel` the panel component owns, so dragging it wider in one
 * tab widens it everywhere, the way a window's furniture should behave.
 */
export const useSidePanelStore = create<SidePanelState>((set) => ({
  items: {},
  opens: 0,

  /**
   * Takes no pane id, and doesn't need one: the tabs that aren't in front are
   * `inert` (see `TabPane`), so a click can only originate in the tab in
   * front — and within it, in the half it landed in, which that click has
   * already focused (the pane's capture-phase handler runs first). That is
   * what keeps every list row in the app calling `openFileSmart` with nothing
   * but a file.
   */
  open: (item) => {
    const pane = activePane();
    if (!pane) return;
    set((s) => ({ items: { ...s.items, [pane.id]: item }, opens: s.opens + 1 }));
  },

  /**
   * For a list that has re-fetched the row it has open — lecture progress
   * ticking over, say. Unlike `open` this names its pane, because the caller
   * can be a page in a tab that is not in front, and it is a no-op unless that
   * pane still has the same item open, so a stale refresh cannot reopen
   * something the user just closed.
   */
  sync: (paneId, item) =>
    set((s) => {
      const cur = s.items[paneId];
      if (!cur || itemKey(cur) !== itemKey(item)) return s;
      return { items: { ...s.items, [paneId]: item } };
    }),

  close: (paneId) =>
    set((s) => {
      if (!s.items[paneId]) return s;
      const next = { ...s.items };
      delete next[paneId];
      return { items: next };
    }),

  dropTab: (paneId) =>
    set((s) => {
      if (!(paneId in s.items)) return s;
      const next = { ...s.items };
      delete next[paneId];
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
  const pane = activePane();
  if (!pane) return;
  const { items, close } = useSidePanelStore.getState();
  const item = items[pane.id];
  if (!item) return;
  if (item.kind === "lecture" && ownsPlayback(pane.id)) stopLecturePlayback();
  close(pane.id);
}

/** The item the panel should be showing: the focused pane's, or nothing. */
export function useActivePanelItem(): PanelItem | null {
  const paneId = useTabStore((s) => {
    const tab = s.tabs.find((t) => t.id === s.activeId);
    return tab ? focusedPane(tab).id : 0;
  });
  return useSidePanelStore((s) => s.items[paneId] ?? null);
}

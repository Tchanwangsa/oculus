import { create } from "zustand";
import type { DbFile, Lecture } from "@/lib/db";
import { ownsPlayback, stopLecturePlayback } from "@/lib/lecturePlayback";
import { activePane, useActivePaneId } from "@/stores/tabStore";

/** What the side panel is showing. One item at a time, per pane — a split
 *  tab's two halves each peek at their own thing. */
export type PanelItem =
  | { kind: "file"; file: DbFile; locate?: FileLocate }
  | { kind: "lecture"; lecture: Lecture };

/** A spot in the file a citation points at: a PDF page (and the passage to
 *  highlight on it), or for markdown just the passage. `seq` is fresh per
 *  click, so citing the same spot again re-jumps. */
export interface FileLocate {
  page?: number;
  quote?: string;
  seq: number;
}

/** Identity within a pane, and the React key that remounts the body. Not
 *  `locate`: another citation into the open file moves it, not reloads it. */
export function itemKey(item: PanelItem): string {
  return item.kind === "file"
    ? `file:${item.file.relative_path}`
    : `lecture:${item.lecture.id}`;
}

interface SidePanelState {
  /** Open item per pane id. A pane with no entry has the panel shut. */
  items: Record<number, PanelItem | undefined>;
  /** Open count. A folded panel unfolds on this, not on the item changing,
   *  so re-opening the item already showing still unfolds it. */
  opens: number;
  open: (item: PanelItem) => void;
  /** Also called when a pane goes away. */
  close: (paneId: number) => void;
  sync: (paneId: number, item: PanelItem) => void;
}

/** The side panel's contents, one entry per pane; the panel is drawn once
 *  and reads the focused pane's. Its size lives in `useResizablePanel`, so it
 *  is shared across tabs. */
export const useSidePanelStore = create<SidePanelState>((set) => ({
  items: {},
  opens: 0,

  /** No pane id needed: background tabs are `inert` (`TabPane`) and the
   *  pane's capture-phase handler has already focused the clicked half. */
  open: (item) => {
    const pane = activePane();
    if (!pane) return;
    set((s) => ({ items: { ...s.items, [pane.id]: item }, opens: s.opens + 1 }));
  },

  /** Refreshes a re-fetched row in place. Names its pane (the caller may be
   *  in a background tab) and no-ops unless that item is still open. */
  sync: (paneId, item) =>
    set((s) => {
      const cur = s.items[paneId];
      if (!cur || itemKey(cur) !== itemKey(item)) return s;
      // A refreshed row keeps the spot it was opened at.
      const next = cur.kind === "file" && item.kind === "file" ? { ...item, locate: item.locate ?? cur.locate } : item;
      return { items: { ...s.items, [paneId]: next } };
    }),

  close: (paneId) =>
    set((s) => {
      if (!s.items[paneId]) return s;
      const next = { ...s.items };
      delete next[paneId];
      return { items: next };
    }),
}));

/** Shuts the focused pane's peek as the shell leaves its page
 *  (`navigateActive`): the peek belongs to the page it was opened from. A
 *  lecture peek is stopped too, as `LecturePanel`'s × does. */
export function closeActivePanel(): void {
  const pane = activePane();
  if (!pane) return;
  const { items, close } = useSidePanelStore.getState();
  const item = items[pane.id];
  if (!item) return;
  if (item.kind === "lecture" && ownsPlayback(pane.id)) stopLecturePlayback();
  close(pane.id);
}

export function useActivePanelItem(): PanelItem | null {
  const paneId = useActivePaneId();
  return useSidePanelStore((s) => s.items[paneId] ?? null);
}

import { create } from "zustand";
import { browseId } from "@/lib/browser";

/**
 * The trail of pages you have had open, most recent first — what the
 * sidebar's Recent group lists.
 *
 * Only the path is kept: the title and icon are derived from it on render
 * (`tabInfo`), so a renamed subject or a re-downloaded lecture follows along
 * instead of leaving a stale label behind. It lives in localStorage rather
 * than SQLite for the same reason `lib/recents.ts` does — throwaway UI state
 * that changes on every click and must be readable on first paint.
 */

const KEY = "oculus-recent-tabs";
/** Kept beyond what the sidebar shows, so closing one reveals the next. */
const LIMIT = 20;

export interface RecentTab {
  path: string;
  visitedAt: number;
}

function read(): RecentTab[] {
  try {
    const raw = localStorage.getItem(KEY);
    const list = raw ? (JSON.parse(raw) as RecentTab[]) : [];
    return Array.isArray(list) ? list.filter((e) => typeof e?.path === "string") : [];
  } catch {
    return [];
  }
}

function write(list: RecentTab[]): void {
  try {
    localStorage.setItem(KEY, JSON.stringify(list));
  } catch {
    /* quota or private mode — the trail is expendable */
  }
}

interface RecentTabsState {
  recents: RecentTab[];
  /** Records a visit, moving an already-seen path back to the front. */
  record: (path: string) => void;
  /** Drops one entry — the × on a Recent row. */
  forget: (path: string) => void;
}

export const useRecentTabsStore = create<RecentTabsState>((set, get) => ({
  recents: read(),

  record: (path) => {
    // A browser tab's path names a native page Rust owns; the id dies with
    // the page, so it would come back as a link to nothing.
    if (browseId(path) != null || path === "/") return;
    const rest = get().recents.filter((e) => e.path !== path);
    const next = [{ path, visitedAt: Date.now() }, ...rest].slice(0, LIMIT);
    write(next);
    set({ recents: next });
  },

  forget: (path) => {
    const next = get().recents.filter((e) => e.path !== path);
    write(next);
    set({ recents: next });
  },
}));

/** Called from the tab strip's navigation effect — the one place that sees
 *  every move the router makes. */
export function recordRecentTab(path: string): void {
  useRecentTabsStore.getState().record(path);
}

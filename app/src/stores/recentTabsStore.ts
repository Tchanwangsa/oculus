import { create } from "zustand";
import { browseId } from "@/lib/browser";

/**
 * The trail of pages you have been on — what the sidebar's Recent group
 * lists.
 *
 * Two rules keep it still enough to read while you work, both of them
 * Notion's:
 *
 * 1. **A page joins the trail only once you have stayed on it** (`DWELL_MS`).
 *    Clicking a subject to reach a lecture inside it is one destination, not
 *    two, and a redirect (`/settings` → `/settings/canvas`) is none.
 * 2. **A page already in the trail never moves.** Coming back to it refreshes
 *    when it was last seen, in place; only a page you have not been to is
 *    inserted, at the top. A list that re-sorted itself on every click was a
 *    list you had to re-read every time you looked at it.
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
/** Long enough to pass through a page on the way to another one. */
const DWELL_MS = 2_500;

export interface RecentTab {
  /** What the entry is *of* — see `recentKey`. One row per thing. */
  key: string;
  path: string;
  visitedAt: number;
}

/**
 * The identity of the thing a path names, or null for a path that is not a
 * destination at all.
 *
 * Coarser than the path on purpose: a subject's nine section tabs are views
 * of one subject, so they share a row that follows you between them rather
 * than filling the group with rows all called "INFO30006". A file and a
 * lecture are the exception — a full page of their own, as their tab is — and
 * so is a task under its project.
 */
export function recentKey(path: string): string | null {
  const [pathname, search = ""] = path.split("?");
  // A browser tab's path names a native page Rust owns; the id dies with
  // the page, so it would come back as a link to nothing. Home is one
  // keystroke away and is where an empty tab starts.
  if (browseId(pathname) != null || pathname === "/") return null;

  const task = /^\/projects\/(\d+)\/tasks\/(\d+)/.exec(pathname);
  if (task) return `task:${task[1]}:${task[2]}`;
  const project = /^\/projects\/(\d+)/.exec(pathname);
  if (project) return `project:${project[1]}`;

  const subject = /^\/subjects\/(\d+)(?:\/([\w-]+))?/.exec(pathname);
  if (subject) {
    const section = subject[2];
    if (section === "file" || section === "lecture") {
      const ref =
        new URLSearchParams(search).get(section === "file" ? "path" : "id") ??
        "";
      return `${section}:${subject[1]}:${ref}`;
    }
    return `subject:${subject[1]}`;
  }

  // Settings' four pages are one place you went, like a subject's tabs.
  if (pathname.startsWith("/settings")) return "settings";
  return pathname;
}

function read(): RecentTab[] {
  try {
    const raw = localStorage.getItem(KEY);
    const list = raw ? (JSON.parse(raw) as RecentTab[]) : [];
    if (!Array.isArray(list)) return [];
    const seen = new Set<string>();
    // Keys are derived rather than trusted: entries written before there were
    // any carry none, and two of them can now collapse into one row.
    return list.flatMap((e) => {
      if (typeof e?.path !== "string") return [];
      const key = recentKey(e.path);
      if (!key || seen.has(key)) return [];
      seen.add(key);
      return [{ key, path: e.path, visitedAt: e.visitedAt ?? 0 }];
    });
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
  /** Records a settled visit. New pages enter at the top; a page already in
   *  the trail is refreshed where it stands. */
  record: (path: string) => void;
  /** Drops one entry — the × on a Recent row. */
  forget: (key: string) => void;
}

export const useRecentTabsStore = create<RecentTabsState>((set, get) => ({
  recents: read(),

  record: (path) => {
    const key = recentKey(path);
    if (!key) return;
    const list = get().recents;
    const seen = { key, path, visitedAt: Date.now() };
    const at = list.findIndex((e) => e.key === key);

    let next: RecentTab[];
    if (at !== -1) {
      next = list.slice();
      next[at] = seen;
    } else {
      next = [seen, ...list];
      // Full: the entry to lose is the one you have gone longest without,
      // which is not the bottom one now that positions are fixed.
      if (next.length > LIMIT) {
        let stalest = 0;
        next.forEach((e, i) => {
          if (e.visitedAt < next[stalest].visitedAt) stalest = i;
        });
        next = next.filter((_, i) => i !== stalest);
      }
    }
    write(next);
    set({ recents: next });
  },

  forget: (key) => {
    const next = get().recents.filter((e) => e.key !== key);
    write(next);
    set({ recents: next });
  },
}));

/** Pending dwell per pane: each tab's router moves on its own, and one pane
 *  navigating must not cancel another pane's arrival. */
const dwelling = new Map<number, number>();

/**
 * Called from a tab pane's navigation effect — the one place that sees every
 * move its router makes. The visit lands only if the pane is still there
 * `DWELL_MS` later.
 */
export function recordRecentTab(paneId: number, path: string): void {
  cancelRecentTab(paneId);
  dwelling.set(
    paneId,
    window.setTimeout(() => {
      dwelling.delete(paneId);
      useRecentTabsStore.getState().record(path);
    }, DWELL_MS),
  );
}

/** Drops a pane's pending visit — it navigated again, or the tab closed. */
export function cancelRecentTab(paneId: number): void {
  const timer = dwelling.get(paneId);
  if (timer !== undefined) clearTimeout(timer);
  dwelling.delete(paneId);
}

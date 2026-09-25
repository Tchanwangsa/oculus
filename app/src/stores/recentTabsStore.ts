import { create } from "zustand";
import { browseId } from "@/lib/browser";

/**
 * The sidebar's Recent trail. A page joins only after `DWELL_MS` on it (so
 * pass-throughs and redirects don't count), and a page already listed never
 * moves — a revisit refreshes it in place; only new pages enter at the top.
 * Only the path is kept; title and icon are derived on render (`tabInfo`).
 */

const KEY = "oculus-recent-tabs";
/** Kept beyond what the sidebar shows, so closing one reveals the next. */
const LIMIT = 20;
const DWELL_MS = 2_500;

export interface RecentTab {
  /** One row per thing — see `recentKey`. */
  key: string;
  path: string;
  visitedAt: number;
}

/** The thing a path names, or null for a non-destination. Coarser than the
 *  path: a subject's section tabs share one row, while a file, lecture or
 *  task gets its own. */
export function recentKey(path: string): string | null {
  const [pathname, search = ""] = path.split("?");
  // A browser tab's id dies with its page; `/new` lists this very trail.
  if (browseId(pathname) != null || pathname === "/" || pathname === "/new")
    return null;

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

  if (pathname.startsWith("/settings")) return "settings";
  return pathname;
}

function read(): RecentTab[] {
  try {
    const raw = localStorage.getItem(KEY);
    const list = raw ? (JSON.parse(raw) as RecentTab[]) : [];
    if (!Array.isArray(list)) return [];
    const seen = new Set<string>();
    // Keys are re-derived, not trusted, so rows `recentKey` merges collapse.
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
  record: (path: string) => void;
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
      // Positions are fixed, so evict the stalest, not the bottom row.
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

/** Pending dwell per pane, so one pane navigating cannot cancel another's. */
const dwelling = new Map<number, number>();

/** Called from a pane's navigation effect; the visit lands if the pane is
 *  still there `DWELL_MS` later. */
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

export function cancelRecentTab(paneId: number): void {
  const timer = dwelling.get(paneId);
  if (timer !== undefined) clearTimeout(timer);
  dwelling.delete(paneId);
}

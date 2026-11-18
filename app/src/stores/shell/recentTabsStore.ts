import { create } from "zustand";
import { browseId } from "@/lib/browser";
import { isGoneRoute } from "@/lib/shell/goneRoutes";
import type { FollowedRoute } from "@/lib/shell/pathFollow";

/**
 * The new-tab page's Recent trail. A page joins only after `DWELL_MS` on it (so
 * pass-throughs and redirects don't count), and a page already listed never
 * moves — a revisit refreshes it in place; only new pages enter at the top.
 * Only the path is kept; title and icon are derived on render (`tabInfo`).
 */

const KEY = "oculus-recent-tabs";
/** Kept beyond what the new-tab page shows. */
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
  // A browser tab's id dies with its page; `/new` lists this very trail;
  // `/subjects` only redirects to a subject, which records itself.
  if (
    isGoneRoute(pathname) ||
    browseId(pathname) != null ||
    pathname === "/" ||
    pathname === "/new" ||
    pathname === "/subjects"
  )
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
  // Drive's folders and Recent are one place, as a subject's tabs are.
  if (pathname === "/drive" || pathname.startsWith("/drive/")) return "drive";
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
  /** Library paths moved or went: a row on a moved path takes the new one
   *  in place (merging into a row already listing it), a row on a deleted
   *  thing goes. */
  follow: (follow: (route: string) => FollowedRoute) => void;
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

  follow: (follow) => {
    const list = get().recents;
    const next = followRecents(list, follow);
    if (next === list) return;
    write(next);
    set({ recents: next });
  },
}));

/** `follow` over the trail, pure. The same array when nothing changed. */
export function followRecents(list: RecentTab[], follow: (route: string) => FollowedRoute): RecentTab[] {
  let changed = false;
  const followed = list.flatMap((e) => {
    const next = follow(e.path);
    if (next.kind === "same") return [e];
    changed = true;
    if (next.kind === "removed") return [];
    const key = recentKey(next.path);
    return key ? [{ key, path: next.path, visitedAt: e.visitedAt }] : [];
  });
  if (!changed) return list;
  // A moved row can land on a thing already listed: the later visit stays,
  // in the earlier row's place.
  const out: RecentTab[] = [];
  for (const e of followed) {
    const at = out.findIndex((o) => o.key === e.key);
    if (at === -1) out.push(e);
    else if (e.visitedAt > out[at].visitedAt) out[at] = e;
  }
  return out;
}

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

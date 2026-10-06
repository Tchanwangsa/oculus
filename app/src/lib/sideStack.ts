import type { PaneSide, PaneState } from "@/stores/tabStore";
import { recentKey } from "@/stores/recentTabsStore";

/**
 * A tab's side panel as plain data: the stack of panes on the tab's right
 * half. `tabStore` holds one per tab and does the side effects (routers,
 * lecture playback); everything here is pure.
 *
 * Items are kept most recently *opened* first. Bringing one to the front only
 * stamps when it was last viewed, so cycling walks a stable list; past
 * `SIDE_CAP` the least recently *viewed* item goes. The front item always
 * carries the highest stamp.
 */

/** How many items a side panel holds. */
export const SIDE_CAP = 8;
/** The main pane's share of the tab when a side panel opens, until a drag of
 *  the seam sets the strip's own (`tabStore`'s `sideRatio`). */
export const SIDE_RATIO = 0.5;
const RATIO_MIN = 0.25;
const RATIO_MAX = 0.75;

export interface SideItem extends PaneState {
  /** Stamp of the last time it came to the front; the cap drops the lowest. */
  viewed: number;
  /** Location state for the first entry of the item's router (a file's
   *  `locate`). The router is built when the item first mounts. */
  entryState?: unknown;
}

export interface SidePanel {
  /** Most recently opened first. */
  items: SideItem[];
  /** The id of the item on show. */
  front: number;
  /** The main pane's share of the tab's width. */
  ratio: number;
}

/** What `oculus-tabs` keeps of a side panel: paths, not routers. */
export interface StoredSide {
  items: { id: number; path: string }[];
  front: number;
  ratio: number;
}

export function clampRatio(ratio: number): number {
  return Math.min(RATIO_MAX, Math.max(RATIO_MIN, ratio));
}

export function frontOf(side: SidePanel): SideItem {
  return side.items.find((i) => i.id === side.front) ?? side.items[0];
}

function nextStamp(items: SideItem[]): number {
  return items.reduce((top, i) => Math.max(top, i.viewed), 0) + 1;
}

export interface Push {
  side: SidePanel;
  /** The item already showing the pushed thing, as it was before the push. */
  hit: SideItem | null;
  /** Items the cap pushed out. */
  dropped: SideItem[];
}

/**
 * Opens `path` beside: the item whose path names the same thing (`recentKey`)
 * comes to the front and the head of the list, otherwise a new item does.
 * Retargeting a hit to `path` is the caller's job — it may have a router.
 * A new panel takes `fresh` as its ratio.
 */
export function pushItem(
  side: SidePanel | null,
  path: string,
  newId: () => number,
  entryState?: unknown,
  fresh = SIDE_RATIO,
): Push {
  const items = side?.items ?? [];
  const ratio = side?.ratio ?? fresh;
  const viewed = nextStamp(items);
  // Null for `/new` and browser pages: those are never the same thing twice.
  const key = recentKey(path);
  const at = key == null ? -1 : items.findIndex((i) => recentKey(i.path) === key);
  if (at !== -1) {
    const hit = items[at];
    return {
      side: {
        items: [{ ...hit, viewed }, ...items.filter((_, i) => i !== at)],
        front: hit.id,
        ratio,
      },
      hit,
      dropped: [],
    };
  }
  let rest = items;
  const dropped: SideItem[] = [];
  while (rest.length >= SIDE_CAP) {
    const stalest = rest.reduce((a, b) => (b.viewed < a.viewed ? b : a));
    dropped.push(stalest);
    rest = rest.filter((i) => i !== stalest);
  }
  const item: SideItem = {
    id: newId(),
    path,
    canBack: false,
    canForward: false,
    viewed,
    entryState,
  };
  return { side: { items: [item, ...rest], front: item.id, ratio }, hit: null, dropped };
}

/** Points an item that has no router yet at `path`, for when it is built. */
export function retargetItem(
  side: SidePanel,
  id: number,
  path: string,
  entryState?: unknown,
): SidePanel {
  return {
    ...side,
    items: side.items.map((i) => (i.id === id ? { ...i, path, entryState } : i)),
  };
}

/** Brings `id` to the front without moving it in the list. */
export function frontItem(side: SidePanel, id: number): SidePanel {
  if (side.front === id || !side.items.some((i) => i.id === id)) return side;
  const viewed = nextStamp(side.items);
  return {
    ...side,
    items: side.items.map((i) => (i.id === id ? { ...i, viewed } : i)),
    front: id,
  };
}

/** The id of the item `delta` places past the front in list order, wrapping
 *  at either end: what ⌃Tab and ⌃⇧Tab bring forward. */
export function stepItem(side: SidePanel, delta: 1 | -1): number {
  const n = side.items.length;
  const at = side.items.indexOf(frontOf(side));
  return side.items[(at + delta + n) % n].id;
}

/** Drops `id`; the most recently viewed item left takes the front. Null once
 *  nothing is left. */
export function removeItem(side: SidePanel, id: number): SidePanel | null {
  if (!side.items.some((i) => i.id === id)) return side;
  const items = side.items.filter((i) => i.id !== id);
  if (items.length === 0) return null;
  if (side.front !== id) return { ...side, items };
  const next = items.reduce((a, b) => (b.viewed > a.viewed ? b : a));
  return { ...side, items, front: next.id };
}

/** A side panel built from paths in list order, the first in front. Stamps
 *  follow the list, so the cap drops from its end. */
function fromPanes(
  panes: { id: number; path: string }[],
  front: number,
  ratio: number,
): SidePanel {
  const items = panes.map((p, i) => ({
    id: p.id,
    path: p.path,
    canBack: false,
    canForward: false,
    viewed: panes.length - i,
  }));
  return frontItem({ items, front: items[0].id, ratio }, front);
}

/** A side panel for ⇧⌘T: fresh ids, the first path in front. */
export function sideFromPaths(
  paths: string[],
  newId: () => number,
  ratio = SIDE_RATIO,
): SidePanel | null {
  if (paths.length === 0) return null;
  const panes = paths.slice(0, SIDE_CAP).map((path) => ({ id: newId(), path }));
  return fromPanes(panes, panes[0].id, ratio);
}

export function storeSide(side: SidePanel): StoredSide {
  return {
    items: side.items.map((i) => ({ id: i.id, path: i.path })),
    front: side.front,
    ratio: side.ratio,
  };
}

function storedPane(p: unknown): p is { id: number; path: string } {
  const o = p as { id?: unknown; path?: unknown } | null;
  return !!o && Number.isInteger(o.id) && typeof o.path === "string";
}

/**
 * A stored tab's side panel. Reads `side` (`StoredSide`), else a lone `split`
 * pane as a one-item panel. Anything malformed — a bad item, an id used
 * twice or equal to the main pane's `mainId` — is no side panel.
 */
export function restoreSide(stored: unknown, split: unknown, mainId: number): SidePanel | null {
  let panes: { id: number; path: string }[];
  let front: unknown;
  let ratio: unknown;
  if (stored != null && typeof stored === "object") {
    const s = stored as Partial<Record<keyof StoredSide, unknown>>;
    if (!Array.isArray(s.items) || !s.items.every(storedPane)) return null;
    panes = s.items.slice(0, SIDE_CAP).map((p) => ({ id: p.id, path: p.path }));
    front = s.front;
    ratio = s.ratio;
  } else if (storedPane(split)) {
    panes = [{ id: split.id, path: split.path }];
  } else {
    return null;
  }
  if (panes.length === 0) return null;
  const ids = new Set([mainId, ...panes.map((p) => p.id)]);
  if (ids.size !== panes.length + 1) return null;
  const frontId = panes.some((p) => p.id === front) ? (front as number) : panes[0].id;
  const r = typeof ratio === "number" && Number.isFinite(ratio) ? clampRatio(ratio) : SIDE_RATIO;
  return fromPanes(panes, frontId, r);
}

/** A stored `focus`: the side panel only if there is one. `"split"` is the
 *  side panel's other stored name. */
export function restoreFocus(focus: unknown, side: SidePanel | null): PaneSide {
  return side && (focus === "side" || focus === "split") ? "side" : "main";
}

/** A closed tab's side panel paths: `side`, else a lone `split` path. */
export function restoreClosedSide(side: unknown, split: unknown): string[] | null {
  if (Array.isArray(side)) {
    const paths = side.filter((p): p is string => typeof p === "string");
    return paths.length > 0 ? paths.slice(0, SIDE_CAP) : null;
  }
  return typeof split === "string" ? [split] : null;
}

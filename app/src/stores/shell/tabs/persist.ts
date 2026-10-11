import {
  clampRatio,
  restoreClosedSide,
  restoreFocus,
  restoreSide,
  SIDE_RATIO,
  storeSide,
  type StoredSide,
} from "@/lib/shell/sideStack";
import { pane, type AppTab } from "./panes";

/** Where a first-run strip opens. */
const FIRST = "/chat";

/** The strip persists across reloads (a memory router restores nothing).
 *  `/browse/<id>` panes are reconciled against Rust by `useBrowserTabs`. */
const STORE_KEY = "oculus-tabs";

/** How far back ⇧⌘T reaches. */
export const CLOSED_LIMIT = 10;

/**
 * A closed tab, kept for ⇧⌘T. A browser tab is remembered by **URL**: its
 * `/browse/<id>` path names a page Rust has destroyed. Which of `path`/`url`
 * is set says which kind it is.
 */
export interface ClosedTab {
  /** Strip index to reopen at; clamped on the way out. */
  index: number;
  /** The route its main pane held, or null for a browser tab. */
  path: string | null;
  url: string | null;
  /** The side panel's routes in list order. Browser items are dropped. */
  side: string[] | null;
}

interface StoredPane {
  id: number;
  path: string;
}

/** Fields are read as untrusted: `restoreSide` validates them. */
interface StoredTab extends StoredPane {
  side?: StoredSide | null;
  /** A one-pane side panel, read when `side` is absent. */
  split?: unknown;
  focus?: unknown;
}

/** A stored closed tab, whose side panel may be a lone `split` path. */
type StoredClosed = Partial<Record<keyof ClosedTab | "split", unknown>>;

interface StoredStrip {
  tabs: StoredTab[];
  activeId: number;
  closed?: StoredClosed[];
  sideRatio?: unknown;
}

/** Generic so filtering a `StoredTab[]` does not narrow its elements down to
 *  the pane fields they share. */
function validPane<T extends StoredPane>(p: T | null | undefined): p is T {
  return !!p && Number.isInteger(p.id) && typeof p.path === "string";
}

function validClosed(e: StoredClosed | null | undefined): boolean {
  return (
    !!e &&
    Number.isInteger(e.index) &&
    (typeof e.path === "string" || typeof e.url === "string")
  );
}

export function restore(): {
  tabs: AppTab[];
  activeId: number;
  closed: ClosedTab[];
  sideRatio: number;
} {
  let closed: ClosedTab[] = [];
  let sideRatio = SIDE_RATIO;
  try {
    const raw = localStorage.getItem(STORE_KEY);
    const saved = raw ? (JSON.parse(raw) as StoredStrip) : null;
    const r = saved?.sideRatio;
    if (typeof r === "number" && Number.isFinite(r)) sideRatio = clampRatio(r);
    closed = (saved?.closed ?? []).filter(validClosed).map((e) => ({
      index: e.index as number,
      path: typeof e.path === "string" ? e.path : null,
      url: typeof e.url === "string" ? e.url : null,
      side: restoreClosedSide(e.side, e.split),
    }));
    const tabs = (saved?.tabs ?? []).filter(validPane).map((t): AppTab => {
      const side = restoreSide(t.side, t.split, t.id);
      return { ...pane(t.id, t.path), side, focus: restoreFocus(t.focus, side) };
    });
    if (tabs.length > 0) {
      const activeId = tabs.some((t) => t.id === saved?.activeId)
        ? saved!.activeId
        : tabs[0].id;
      return { tabs, activeId, closed, sideRatio };
    }
  } catch {
    /* corrupt or unavailable — a fresh strip is a fine fallback */
  }
  return {
    tabs: [{ ...pane(1, FIRST), side: null, focus: "main" }],
    activeId: 1,
    closed,
    sideRatio,
  };
}

/** Writes the strip to localStorage; the store calls it on every change. */
export function saveStrip(s: {
  tabs: AppTab[];
  activeId: number;
  closed: ClosedTab[];
  sideRatio: number;
}): void {
  try {
    const strip: StoredStrip = {
      tabs: s.tabs.map((t) => ({
        id: t.id,
        path: t.path,
        side: t.side && storeSide(t.side),
        focus: t.focus,
      })),
      activeId: s.activeId,
      closed: s.closed,
      sideRatio: s.sideRatio,
    };
    localStorage.setItem(STORE_KEY, JSON.stringify(strip));
  } catch {
    /* quota or private mode — the strip is expendable */
  }
}

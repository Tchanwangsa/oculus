import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";

/**
 * The in-app browser. Every tab is a native WebView Rust owns
 * (`app/src-tauri/src/shell/browser/`), mirrored into the tab strip; a
 * `/browse/:id` route leaves an empty slot and reports where it is. Placement
 * is per page — Rust never hides one page to show another, so this side
 * decides which slots show what.
 */

export interface BrowserTab {
  id: number;
  url: string;
  title: string;
  loading: boolean;
  /** From the page's own back/forward list in Rust. */
  can_back: boolean;
  can_forward: boolean;
  /** The page's zoom, 1 = 100%. */
  zoom: number;
}

export interface FaviconFound {
  host: string;
  /** A `data:` URL. */
  icon: string;
}

/** No match *count*: WebKit's find API answers with a boolean only. */
export interface FindResult {
  id: number;
  query: string;
  found: boolean;
}

export interface BrowserSnapshot {
  tabs: BrowserTab[];
}

/** Where the page goes: insets from the window's edges, in logical points. */
export interface Viewport {
  left: number;
  top: number;
  right: number;
  bottom: number;
  /** Radius of the page's bottom corners, matching the card's. */
  radius: number;
}

const BROWSE_PREFIX = "/browse/";

/** The range `shell/browser/page.rs` clamps to, repeated so the toolbar can disable
 *  its buttons at the ends. */
const ZOOM_MIN = 0.5;
const ZOOM_MAX = 3;
/** ⌘= steps, so zoom lands on round numbers. */
const ZOOM_STEPS = [0.5, 0.67, 0.75, 0.8, 0.9, 1, 1.1, 1.25, 1.5, 1.75, 2, 2.5, 3];

export interface SearchEngine {
  id: string;
  label: string;
  /** Where a browser tab with no destination starts. */
  home: string;
  /** Prefix a URL-encoded query is appended to. */
  query: string;
}

export const SEARCH_ENGINES: SearchEngine[] = [
  {
    id: "duckduckgo",
    label: "DuckDuckGo",
    home: "https://duckduckgo.com",
    query: "https://duckduckgo.com/?q=",
  },
  {
    id: "google",
    label: "Google",
    home: "https://www.google.com",
    query: "https://www.google.com/search?q=",
  },
  {
    id: "bing",
    label: "Bing",
    home: "https://www.bing.com",
    query: "https://www.bing.com/search?q=",
  },
  {
    id: "brave",
    label: "Brave Search",
    home: "https://search.brave.com",
    query: "https://search.brave.com/search?q=",
  },
  {
    id: "kagi",
    label: "Kagi",
    home: "https://kagi.com",
    query: "https://kagi.com/search?q=",
  },
];

/** The engine in force, pushed in by `app/src/stores/shell/browserPrefsStore.ts`
 *  so synchronous callers can read it; importing the store here would form
 *  a cycle. */
let engine: SearchEngine = SEARCH_ENGINES[0];

export function setSearchEngine(id: string): void {
  engine = SEARCH_ENGINES.find((e) => e.id === id) ?? SEARCH_ENGINES[0];
}

export function searchEngine(): SearchEngine {
  return engine;
}

/** Where a browser tab with no destination starts. */
export function searchHome(): string {
  return engine.home;
}

/** Send every link to the system browser. A module value, like `engine`,
 *  because click handlers cannot await a database read. */
let openLinksInSystem = false;

export function setOpenLinksInSystem(value: boolean): void {
  openLinksInSystem = value;
}

/** A browser tab's route, stable for its life: navigation changes the URL
 *  in Rust, never the route. */
export function browsePath(id: number): string {
  return `${BROWSE_PREFIX}${id}`;
}

/** The browser tab id a route names, or null for an app route. */
export function browseId(path: string | undefined): number | null {
  if (!path?.startsWith(BROWSE_PREFIX)) return null;
  const id = Number(path.slice(BROWSE_PREFIX.length).split(/[?#]/)[0]);
  return Number.isInteger(id) ? id : null;
}

export function isWebUrl(href: string | null | undefined): href is string {
  return !!href && /^https?:\/\//i.test(href);
}

/** Whether typed text is an address or a search — also labels the address
 *  bar's "Go to" / "Search for" rows. */
export function addressKind(input: string): "url" | "query" {
  const text = input.trim();
  if (/^https?:\/\//i.test(text)) return "url";
  // A bare host or path is an address; anything with a space is a query.
  if (/^[\w-]+(\.[\w-]+)+(\/|$|\?|#)/.test(text)) return "url";
  return "query";
}

export function normalizeAddress(input: string): string {
  const text = input.trim();
  if (!text) return "";
  if (addressKind(text) === "url") {
    return /^https?:\/\//i.test(text) ? text : `https://${text}`;
  }
  return `${engine.query}${encodeURIComponent(text)}`;
}

export function hostOf(url: string): string {
  try {
    return new URL(url).host.replace(/^www\./, "");
  } catch {
    return "";
  }
}

export const browser = {
  /** Opens a new tab; it reaches the strip via `browser-state`. */
  open: (url: string) => invoke<number>("browser_open_url", { url }),
  external: (url: string) => openUrl(url),
  state: () => invoke<BrowserSnapshot>("browser_state"),
  /** Shows one page in `viewport`, leaving every other page as it is. */
  place: (id: number, viewport: Viewport) =>
    invoke("browser_place", { id, viewport }),
  setViewport: (id: number, viewport: Viewport) =>
    invoke("browser_set_viewport", { id, viewport }),
  /** Takes one page off screen; it keeps its history position. */
  hideTab: (id: number) => invoke("browser_hide_tab", { id }),
  /** A PNG of the page, painted in its slot while the DOM draws over it —
   *  nothing renders on top of a native page. Rejects when there is no still;
   *  callers then just hide the page. */
  snapshot: (id: number) =>
    invoke<ArrayBuffer>("browser_snapshot", { id }),
  hide: () => invoke("browser_hide"),
  navigate: (id: number, url: string) =>
    invoke("browser_navigate", { id, url }),
  history: (id: number, delta: number) =>
    invoke("browser_history", { id, delta }),
  /** `hard` bypasses the cache (`reloadFromOrigin`); a plain reload serves
   *  stale cached responses. */
  reload: (id: number, hard = false) =>
    invoke("browser_reload", { id, hard }),
  /** Clamped in Rust; the result arrives in the next snapshot. */
  setZoom: (id: number, zoom: number) =>
    invoke("browser_set_zoom", { id, zoom }),
  /** The result arrives as a `browser-find` event. */
  find: (id: number, query: string, backwards = false) =>
    invoke("browser_find", { id, query, backwards }),
  findClear: (id: number) => invoke("browser_find_clear", { id }),
  close: (id: number) => invoke("browser_close_tab", { id }),
};

export function stepZoom(current: number, direction: 1 | -1): number {
  const steps = ZOOM_STEPS;
  if (direction > 0) return steps.find((z) => z > current + 0.001) ?? ZOOM_MAX;
  return [...steps].reverse().find((z) => z < current - 0.001) ?? ZOOM_MIN;
}

/**
 * Where every external link ends up (`AppLayout`'s capture-phase handler and
 * direct callers). The click was already `preventDefault`-ed, so a failed
 * in-app tab falls back to the system browser. `system` (⌘-click) skips the
 * in-app tab, as does the Settings → Browser preference.
 */
export async function openExternal(url: string, system = false): Promise<void> {
  if (!system && !openLinksInSystem) {
    try {
      await browser.open(url);
      return;
    } catch (e) {
      console.error(
        `[oculus] in-app tab failed for ${url} — opening it in the real browser instead`,
        e,
      );
    }
  }
  try {
    await browser.external(url);
  } catch (e) {
    console.error(`[oculus] could not open ${url} anywhere`, e);
  }
}

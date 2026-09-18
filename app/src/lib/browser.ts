import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";

/**
 * The in-app browser, as the frontend sees it. Every tab is a native page
 * WebView that Rust owns (`app/src-tauri/src/browser.rs`); the frontend
 * mirrors Rust's tab list into the top tab strip, and whatever wants to show
 * a page — the `/browse/:id` route, in the content card — leaves an empty
 * slot for it and reports where that slot is.
 *
 * Placement and visibility are per page: `place` shows one page and moves no
 * other, `hideTab` takes one back down. Rust never infers that showing one
 * page means hiding another, so two slots can hold two pages at once and this
 * side is the one that knows which.
 */

export interface BrowserTab {
  id: number;
  url: string;
  title: string;
  loading: boolean;
  /** Read off the page's own back/forward list in Rust — the only place that
   *  can know. The arrows grey out on these rather than staying lit. */
  can_back: boolean;
  can_forward: boolean;
  /** The page's zoom, 1 = 100%. */
  zoom: number;
}

/** A site's icon arriving from Rust, one host at a time. */
export interface FaviconFound {
  host: string;
  /** A `data:` URL. */
  icon: string;
}

/** What a find landed on. There is no match *count*: WebKit's find API
 *  answers with a boolean and nothing else (`browser.rs`). */
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

/** How far a page may be zoomed — the range `browser.rs` clamps to, repeated
 *  here so the toolbar can grey its buttons out at the ends rather than
 *  offering a step that does nothing. */
export const ZOOM_MIN = 0.5;
export const ZOOM_MAX = 3;
/** The steps ⌘= walks through, so zoom lands on round numbers instead of
 *  drifting by repeated multiplication. */
export const ZOOM_STEPS = [0.5, 0.67, 0.75, 0.8, 0.9, 1, 1.1, 1.25, 1.5, 1.75, 2, 2.5, 3];

export interface SearchEngine {
  id: string;
  label: string;
  /** Where a browser tab with no destination starts. */
  home: string;
  /** Prefix a URL-encoded query is appended to. */
  query: string;
}

/** What the address bar can search with. A small list on purpose: this is a
 *  student's coursework browser, not a place to configure a custom engine
 *  with a `%s` in it. */
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

/**
 * The engine in force, kept as a module value rather than read from a store.
 *
 * `normalizeAddress` is called from render paths and from `search.ts`'s
 * ranking, both of which are synchronous, and the setting lives in SQLite.
 * So the preference store loads it once at startup and pushes it here
 * (`app/src/stores/browserPrefsStore.ts`); this module never reads the
 * database and never imports the store, which is what keeps the two from
 * forming a cycle.
 */
let engine: SearchEngine = SEARCH_ENGINES[0];

export function setSearchEngine(id: string): void {
  engine = SEARCH_ENGINES.find((e) => e.id === id) ?? SEARCH_ENGINES[0];
}

export function searchEngine(): SearchEngine {
  return engine;
}

/** Where a browser tab opened with no destination starts: the configured
 *  engine's home, so the new-tab page's browser door and the address bar
 *  cannot disagree about which search this app does. */
export function searchHome(): string {
  return engine.home;
}

/** Whether every link should leave for the real browser instead of opening a
 *  tab here. Same reason as `engine`: `openExternal` is called from click
 *  handlers that cannot await a database read. */
let openLinksInSystem = false;

export function setOpenLinksInSystem(value: boolean): void {
  openLinksInSystem = value;
}

/** The route a browser tab lives at. Stable for the tab's life: page
 *  navigations change the tab's URL in Rust, never the route. */
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

/** Whether what was typed reads as an address or as something to look up.
 *  Split out from `normalizeAddress` because the answer matters on its own:
 *  the address bar labels a row "Go to" or "Search for" by it, and
 *  `search.ts` used to infer the same thing by looking for `?q=` in the
 *  result — which stopped being a safe tell the moment the engine became a
 *  setting. */
export function addressKind(input: string): "url" | "query" {
  const text = input.trim();
  if (/^https?:\/\//i.test(text)) return "url";
  // A bare host or path is an address; anything with a space is a query.
  if (/^[\w-]+(\.[\w-]+)+(\/|$|\?|#)/.test(text)) return "url";
  return "query";
}

/** What the user typed in the address bar: a URL, or something to search for. */
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
  /** Opens `url` in a new tab. The tab reaches the strip via `browser-state`. */
  open: (url: string) => invoke<number>("browser_open_url", { url }),
  /** The escape hatch: hand the URL to the real browser. */
  external: (url: string) => openUrl(url),
  state: () => invoke<BrowserSnapshot>("browser_state"),
  /** Show tab `id`'s page in the slot `viewport` describes, leaving every
   *  other page where and as it is. */
  place: (id: number, viewport: Viewport) =>
    invoke("browser_place", { id, viewport }),
  /** Move one page's slot without changing whether it is showing. */
  setViewport: (id: number, viewport: Viewport) =>
    invoke("browser_set_viewport", { id, viewport }),
  /** Take one page off screen: its slot is gone, or something has to draw
   *  over it. The page keeps its position in history. */
  hideTab: (id: number) => invoke("browser_hide_tab", { id }),
  /** A PNG of what tab `id`'s page is showing right now, for the app to
   *  paint in the slot while it draws over it — the DOM cannot render on top
   *  of a native page, so a popover over one is a popover over a still of it.
   *  Rejects when there is no still to be had; the caller's fallback is to
   *  hide the page and show nothing, which is what used to happen always. */
  snapshot: (id: number) =>
    invoke<ArrayBuffer>("browser_snapshot", { id }),
  /** Every page off screen at once, for teardown. */
  hide: () => invoke("browser_hide"),
  navigate: (id: number, url: string) =>
    invoke("browser_navigate", { id, url }),
  history: (id: number, delta: number) =>
    invoke("browser_history", { id, delta }),
  /** `hard` reloads ignoring the cache (`reloadFromOrigin`). A plain
   *  reload obeys it, which is how a fixed bug goes on reproducing. */
  reload: (id: number, hard = false) =>
    invoke("browser_reload", { id, hard }),
  /** Page zoom, clamped in Rust. The answer comes back in the next snapshot
   *  rather than from here — Rust owns what a tab holds. */
  setZoom: (id: number, zoom: number) =>
    invoke("browser_set_zoom", { id, zoom }),
  /** Find and select the next hit. Whether anything matched arrives as a
   *  `browser-find` event, not as a return value. */
  find: (id: number, query: string, backwards = false) =>
    invoke("browser_find", { id, query, backwards }),
  /** Drop the find's selection, for when the bar closes. */
  findClear: (id: number) => invoke("browser_find_clear", { id }),
  close: (id: number) => invoke("browser_close_tab", { id }),
};

/** One step along `ZOOM_STEPS` from wherever the page is now. */
export function stepZoom(current: number, direction: 1 | -1): number {
  const steps = ZOOM_STEPS;
  if (direction > 0) return steps.find((z) => z > current + 0.001) ?? ZOOM_MAX;
  return [...steps].reverse().find((z) => z < current - 0.001) ?? ZOOM_MIN;
}

/**
 * Where every external link in the app ends up — the capture-phase handler in
 * `AppLayout` calls this, and so does anything that opens a URL on purpose.
 *
 * It has a fallback because the click that got here was already
 * `preventDefault`-ed: if opening the in-app tab fails there is no default
 * navigation left to happen, and a swallowed rejection made the link look
 * inert with nothing said about why. So a failed tab hands the URL to the
 * real browser and says what went wrong in the console — the link always goes
 * somewhere.
 *
 * `system` is the ⌘-click path: skip the in-app tab and leave for the real
 * browser directly — and so is the Settings → Browser preference below, for
 * anyone who would rather every link left Oculus.
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

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

/** What the user typed in the address bar: a URL, or something to search for. */
export function normalizeAddress(input: string): string {
  const text = input.trim();
  if (!text) return "";
  if (/^https?:\/\//i.test(text)) return text;
  // A bare host or path is an address; anything with a space is a query.
  if (/^[\w-]+(\.[\w-]+)+(\/|$|\?|#)/.test(text)) return `https://${text}`;
  return `https://duckduckgo.com/?q=${encodeURIComponent(text)}`;
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
  /** Every page off screen at once, for teardown. */
  hide: () => invoke("browser_hide"),
  navigate: (id: number, url: string) =>
    invoke("browser_navigate", { id, url }),
  history: (id: number, delta: number) =>
    invoke("browser_history", { id, delta }),
  reload: (id: number) => invoke("browser_reload", { id }),
  close: (id: number) => invoke("browser_close_tab", { id }),
};

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
 * browser directly.
 */
export async function openExternal(url: string, system = false): Promise<void> {
  if (!system) {
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

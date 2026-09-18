import { useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  browser,
  browseId,
  browsePath,
  hostOf,
  type BrowserSnapshot,
  type FaviconFound,
} from "@/lib/browser";
import {
  loadFavicons,
  recordTitle,
  recordVisit,
  saveFavicon,
} from "@/lib/browserHistory";
import { useBrowserStore } from "@/stores/browserStore";
import { useBrowserPrefsStore } from "@/stores/browserPrefsStore";
import { panesOf, useTabStore } from "@/stores/tabStore";

/**
 * Keeps the tab strip in step with Rust's browser tab list. Rust is the
 * source of truth for which browser tabs exist; the strip is the source of
 * truth for their order among the app's own tabs and which one is in front.
 * So every snapshot is reconciled one way: a page Rust has that no pane is
 * showing is opened as a tab in front (a link was clicked, or a page popped
 * one), and a pane whose page Rust no longer has is closed.
 *
 * Panes, not tabs, because a split half can hold a page too — that is half of
 * what splitting is for. A page adopted into a split is therefore *not* given
 * a tab of its own, and a split navigated away from its page hands that page
 * back to the strip on the next snapshot, which is what keeps a page from
 * being stranded with nothing showing it.
 *
 * It is also where **history is written**. Rust sees the page loads, but the
 * snapshot carries everything a visit is — the URL as it commits, the title
 * once WebKit has parsed it — and the browser tables belong to the frontend
 * the way every other table does.
 *
 * Two rules keep that honest. A visit is recorded when the page has
 * **finished** loading, never when it commits: a commit fires for every hop of
 * a redirect chain, and one Canvas SSO bounce would otherwise leave four rows
 * for one destination. And the last URL recorded per tab is remembered here,
 * so a snapshot fired for something else — a title landing, a sibling tab
 * loading — does not count the same page twice.
 *
 * Mounted once, in AppLayout: it is the strip it reconciles, not any one
 * page.
 */
export function useBrowserTabs() {
  /** The URL each tab was last recorded at, so a visit is counted on the
   *  change and not on every snapshot that mentions the tab. */
  const seen = useRef(new Map<number, string>());
  /** URLs whose title has been written, for the same reason: WebKit reports a
   *  title change more than once for a page that sets one from script. */
  const titled = useRef(new Set<string>());

  useEffect(() => {
    let cancelled = false;

    const remember = (snapshot: BrowserSnapshot) => {
      const live = new Set(snapshot.tabs.map((t) => t.id));
      for (const id of seen.current.keys())
        if (!live.has(id)) seen.current.delete(id);

      for (const tab of snapshot.tabs) {
        // Still in flight: this URL may be one hop of a redirect chain and not
        // anywhere you ended up.
        if (tab.loading) continue;
        if (seen.current.get(tab.id) !== tab.url) {
          seen.current.set(tab.id, tab.url);
          titled.current.delete(tab.url);
          void recordVisit(tab.url, tab.title).catch(() => {});
          continue;
        }
        // Same page, and now it has a name.
        if (tab.title && !titled.current.has(tab.url)) {
          titled.current.add(tab.url);
          void recordTitle(tab.url, tab.title).catch(() => {});
        }
      }
    };

    const reconcile = (snapshot: BrowserSnapshot) => {
      useBrowserStore.getState().apply(snapshot);
      remember(snapshot);
      const live = new Set(snapshot.tabs.map((t) => t.id));

      // Closed in Rust: drop the pane showing it. For a main pane that is the
      // tab — `closeTab` picks the neighbour, already sitting at its own path,
      // and sends the last tab home. For a split half it is only that half,
      // which folds and leaves the tab where it was.
      for (const tab of useTabStore.getState().tabs) {
        const main = browseId(tab.path);
        if (main != null && !live.has(main)) {
          useTabStore.getState().closeTab(tab.id);
          continue;
        }
        const split = browseId(tab.split?.path);
        if (split != null && !live.has(split))
          useTabStore.getState().closeSplit(tab.id);
      }

      // Opened in Rust: a new strip tab, in front. Opening it *is* the
      // navigation — the pane is created at its route and comes forward.
      // A page a split half already holds is not new and gets no tab.
      const known = new Set(
        useTabStore
          .getState()
          .tabs.flatMap((t) => panesOf(t).map((p) => browseId(p.path)))
          .filter((id): id is number => id != null),
      );
      for (const tab of snapshot.tabs) {
        if (known.has(tab.id)) continue;
        useTabStore.getState().addTab(browsePath(tab.id));
      }
    };

    // Ask once on mount (a dev reload has missed every event so far), then
    // follow every change.
    browser
      .state()
      .then((s) => {
        if (!cancelled) reconcile(s);
      })
      .catch(() => {});
    const unlisten = listen<BrowserSnapshot>("browser-state", (e) => {
      if (!cancelled) reconcile(e.payload);
    });

    // Icons found beside a page load. Kept in the store for this session and
    // in the database for the next one; Rust asks the network once per host
    // per run, so without the second half every restart would re-fetch every
    // icon.
    const unlistenIcon = listen<FaviconFound>("browser-favicon", (e) => {
      if (cancelled) return;
      const { host, icon } = e.payload;
      useBrowserStore.getState().setFavicon(host, icon);
      void saveFavicon(host, icon).catch(() => {});
    });

    loadFavicons()
      .then((icons) => {
        if (!cancelled) useBrowserStore.getState().seedFavicons(icons);
      })
      .catch(() => {});
    void useBrowserPrefsStore.getState().load().catch(() => {});

    return () => {
      cancelled = true;
      unlisten.then((off) => off()).catch(() => {});
      unlistenIcon.then((off) => off()).catch(() => {});
    };
  }, []);
}

/** The icon for whatever site a URL is on, or undefined while none is known.
 *  Hosts are what icons are keyed by, so this is the only lookup there is. */
export function faviconFor(
  url: string | undefined,
  icons: Record<string, string>,
): string | undefined {
  if (!url) return undefined;
  try {
    return icons[new URL(url).host];
  } catch {
    return undefined;
  }
}

/** `hostOf` strips `www.`; the icon map does not, since Rust keys it on the
 *  host the page was actually served from. Exported for the callers that only
 *  have a display host to go on. */
export function faviconForHost(
  host: string,
  icons: Record<string, string>,
): string | undefined {
  return icons[host] ?? icons[`www.${host}`] ?? icons[hostOf(`https://${host}`)];
}

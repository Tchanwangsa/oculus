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
} from "@/lib/browser/history";
import { useBrowserStore } from "@/stores/shell/browserStore";
import { useBrowserPrefsStore } from "@/stores/shell/browserPrefsStore";
import { panesOf, useTabStore } from "@/stores/shell/tabStore";

/**
 * Reconciles the tab strip with Rust's browser tab list (Rust owns which
 * pages exist; the strip owns order and focus): a page no pane shows opens as
 * a tab in front, and a pane whose page is gone closes. Works per pane, so a
 * page held by a side panel item, in front or not, gets no tab of its own.
 * A tab Rust closes while it is in front (one opened only to download) hands
 * the front back to the tab it was opened from, not to its neighbour.
 *
 * Also writes history from the snapshots — on **finished** loads only, since
 * a commit fires for every redirect hop. Mounted once, in AppLayout.
 */
export function useBrowserTabs() {
  /** Last URL recorded per tab, so a visit counts once per change. */
  const seen = useRef(new Map<number, string>());
  /** URLs whose title is written — WebKit repeats scripted title changes. */
  const titled = useRef(new Set<string>());
  /** The strip tab in front when each page was given a tab of its own. */
  const openedFrom = useRef(new Map<number, number>());

  useEffect(() => {
    let cancelled = false;

    const remember = (snapshot: BrowserSnapshot) => {
      const live = new Set(snapshot.tabs.map((t) => t.id));
      for (const id of seen.current.keys())
        if (!live.has(id)) seen.current.delete(id);

      for (const tab of snapshot.tabs) {
        if (tab.loading) continue;
        if (seen.current.get(tab.id) !== tab.url) {
          seen.current.set(tab.id, tab.url);
          titled.current.delete(tab.url);
          void recordVisit(tab.url, tab.title).catch(() => {});
          continue;
        }
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

      // Closed in Rust: close the tab, or drop just the side panel item.
      for (const tab of useTabStore.getState().tabs) {
        const main = browseId(tab.path);
        if (main != null && !live.has(main)) {
          const { tabs, activeId, setActive, closeTab } = useTabStore.getState();
          const from = openedFrom.current.get(main);
          if (activeId === tab.id && from != null && tabs.some((t) => t.id === from)) setActive(from);
          closeTab(tab.id);
          continue;
        }
        for (const item of tab.side?.items ?? []) {
          const page = browseId(item.path);
          if (page != null && !live.has(page))
            useTabStore.getState().removeSide(tab.id, item.id);
        }
      }

      // Opened in Rust: a new tab in front, unless a pane already holds it.
      const known = new Set(
        useTabStore
          .getState()
          .tabs.flatMap((t) => panesOf(t).map((p) => browseId(p.path)))
          .filter((id): id is number => id != null),
      );
      for (const id of openedFrom.current.keys())
        if (!live.has(id)) openedFrom.current.delete(id);
      for (const tab of snapshot.tabs) {
        if (known.has(tab.id)) continue;
        openedFrom.current.set(tab.id, useTabStore.getState().activeId);
        useTabStore.getState().addTab(browsePath(tab.id));
      }
    };

    // Ask once on mount (a reload has missed every event), then follow.
    browser
      .state()
      .then((s) => {
        if (!cancelled) reconcile(s);
      })
      .catch(() => {});
    const unlisten = listen<BrowserSnapshot>("browser-state", (e) => {
      if (!cancelled) reconcile(e.payload);
    });

    // Persisted too: Rust fetches each host's icon only once per run.
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

/** For a display host from `hostOf`, which strips the `www.` the icon map
 *  (keyed on the served host) keeps. */
export function faviconForHost(
  host: string,
  icons: Record<string, string>,
): string | undefined {
  return icons[host] ?? icons[`www.${host}`] ?? icons[hostOf(`https://${host}`)];
}

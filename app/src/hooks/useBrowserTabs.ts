import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { browser, browseId, browsePath, type BrowserSnapshot } from "@/lib/browser";
import { useBrowserStore } from "@/stores/browserStore";
import { useTabStore } from "@/stores/tabStore";

/**
 * Keeps the tab strip in step with Rust's browser tab list. Rust is the
 * source of truth for which browser tabs exist; the strip is the source of
 * truth for their order among the app's own tabs and which one is in front.
 * So every snapshot is reconciled one way: a tab Rust has that the strip
 * lacks is opened in front (a link was clicked, or a page popped one), and
 * a strip tab whose page Rust no longer has is closed.
 *
 * Mounted once, in AppLayout: it is the strip it reconciles, not any one
 * page.
 */
export function useBrowserTabs() {
  useEffect(() => {
    let cancelled = false;

    const reconcile = (snapshot: BrowserSnapshot) => {
      useBrowserStore.getState().apply(snapshot);
      const live = new Set(snapshot.tabs.map((t) => t.id));

      // Closed in Rust: drop it from the strip. `closeTab` picks the
      // neighbour — already sitting at its own path — and, for the last tab,
      // sends it home.
      for (const tab of useTabStore.getState().tabs) {
        const id = browseId(tab.path);
        if (id == null || live.has(id)) continue;
        useTabStore.getState().closeTab(tab.id);
      }

      // Opened in Rust: a new strip tab, in front. Opening it *is* the
      // navigation — the pane is created at its route and comes forward.
      const known = new Set(
        useTabStore
          .getState()
          .tabs.map((t) => browseId(t.path))
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
    return () => {
      cancelled = true;
      unlisten.then((off) => off());
    };
  }, []);
}

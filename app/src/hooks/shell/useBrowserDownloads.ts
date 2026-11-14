import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { browseId } from "@/lib/browser";
import { downloads } from "@/lib/browser/downloads/commands";
import { nextDeadline, type View } from "@/lib/browser/downloads/prompt";
import { createSavedRecorder } from "@/lib/browser/downloads/recordSaved";
import type { DownloadsSnapshot } from "@/lib/browser/downloads/types";
import { getFileByRelativePath, getSubjects } from "@/lib/db";
import { recordUploads } from "@/lib/files/uploads";
import { useBrowserDownloadsStore } from "@/stores/shell/browserDownloadsStore";
import { useBrowserStore } from "@/stores/shell/browserStore";
import { sideFront, useTabStore, type AppTab } from "@/stores/shell/tabStore";

/** The browser tabs on show in the tab in front: its main pane's, and the
 *  side panel's front item's while the panel is open. */
export function frontBrowserTabs(tab: AppTab | undefined): number[] {
  if (!tab) return [];
  const side = tab.side && !tab.side.hidden ? sideFront(tab) : null;
  return [browseId(tab.path), browseId(side?.path)].filter((id): id is number => id != null);
}

function currentView(): View {
  const { tabs, activeId } = useTabStore.getState();
  const front = frontBrowserTabs(tabs.find((t) => t.id === activeId));
  const browser = useBrowserStore.getState();
  // Before the first browser snapshot nothing is known to have closed.
  const live = new Set(browser.loaded ? browser.tabs.map((t) => t.id) : front);
  return { front, live };
}

function sameView(a: View, b: View): boolean {
  return (
    a.front.length === b.front.length &&
    a.front.every((t, i) => t === b.front[i]) &&
    a.live.size === b.live.size &&
    [...a.live].every((t) => b.live.has(t))
  );
}

/**
 * Mirrors Rust's download list into `browserDownloadsStore`, tells it which
 * browser tabs are in front, and records each download saved in Oculus as an
 * upload, once (`recordSaved.ts`). Mounted once, in AppLayout.
 */
export function useBrowserDownloads() {
  useEffect(() => {
    let cancelled = false;
    let heard = false;
    let timer = 0;
    const store = useBrowserDownloadsStore;

    const record = createSavedRecorder({
      subjectOf: async (code) => (await getSubjects()).find((s) => s.code === code) ?? null,
      hasRow: async (path) => (await getFileByRelativePath(path)) != null,
      record: recordUploads,
    });

    // An undecided arrival is decided on a tick once its wait runs out.
    const schedule = () => {
      window.clearTimeout(timer);
      const due = nextDeadline(store.getState().prompt);
      if (due == null) return;
      timer = window.setTimeout(() => {
        store.getState().tick(Date.now());
        schedule();
      }, Math.max(0, due - Date.now()) + 1);
    };

    const take = (snapshot: DownloadsSnapshot, initial: boolean) => {
      store.getState().apply(snapshot, Date.now(), initial);
      schedule();
      void record(snapshot.downloads);
    };

    const syncView = () => {
      const view = currentView();
      if (sameView(view, store.getState().view)) return;
      store.getState().setView(view, Date.now());
      schedule();
    };
    syncView();
    const offTabs = useTabStore.subscribe(syncView);
    const offBrowser = useBrowserStore.subscribe(syncView);

    // Ask once on mount (a reload has missed every event), then follow. A
    // list fetched after an event arrived would be the older of the two.
    downloads
      .list()
      .then((s) => {
        if (!cancelled && !heard) take(s, true);
      })
      .catch(() => {});
    const unlisten = listen<DownloadsSnapshot>("browser-downloads", (e) => {
      if (cancelled) return;
      heard = true;
      take(e.payload, false);
    });

    return () => {
      cancelled = true;
      window.clearTimeout(timer);
      offTabs();
      offBrowser();
      unlisten.then((off) => off()).catch(() => {});
    };
  }, []);
}

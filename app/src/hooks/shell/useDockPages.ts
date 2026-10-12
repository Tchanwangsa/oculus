import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { dock, type DockSnapshot, type InboxItem } from "@/lib/dock";
import { useDockStore } from "@/stores/shell/dockStore";
import { useDockPrefsStore } from "@/stores/shell/dockPrefsStore";

/**
 * Opens each enabled dock service's page at startup (Rust keeps no record of
 * which are on) and mirrors Rust's `dock-state` and Inbox (`dock_inbox`, then
 * each `dock-inbox` arrival) into `dockStore`. Enabling one already open is a
 * no-op in Rust, so a webview reload is harmless.
 * Mounted once, in AppLayout.
 */
export function useDockPages() {
  useEffect(() => {
    let cancelled = false;
    const apply = (snapshot: DockSnapshot) => {
      if (!cancelled) useDockStore.getState().apply(snapshot);
    };

    const unlisten = listen<DockSnapshot>("dock-state", (e) => apply(e.payload));
    const unlistenInbox = listen<InboxItem>("dock-inbox", (e) => {
      if (!cancelled) useDockStore.getState().receive(e.payload);
    });
    dock
      .inbox()
      .then((items) => {
        if (!cancelled) useDockStore.getState().loadInbox(items);
      })
      .catch((e) => console.error("[oculus] dock: could not load the Inbox", e));

    void (async () => {
      const prefs = useDockPrefsStore.getState();
      await prefs.load();
      await Promise.all(
        useDockPrefsStore.getState().enabled.map((id) =>
          dock
            .setEnabled(id, true)
            .then(apply)
            .catch((e) => console.error(`[oculus] dock: could not open ${id}`, e)),
        ),
      );
      apply(await dock.list());
    })().catch((e) => console.error("[oculus] dock: startup failed", e));

    return () => {
      cancelled = true;
      unlisten.then((off) => off()).catch(() => {});
      unlistenInbox.then((off) => off()).catch(() => {});
    };
  }, []);
}

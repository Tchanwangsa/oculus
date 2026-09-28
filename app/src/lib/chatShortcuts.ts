import { listen } from "@tauri-apps/api/event";
import { activePane } from "@/stores/tabStore";

/** Every Chat pane stays mounted, but the native menu shortcut belongs only
 * to the focused one. Check at delivery, not when the subscription mounts. */
export function listenConversationToggle(paneId: number, toggle: () => void): () => void {
  let cancelled = false;
  const subscription = listen("menu-toggle-conversations", () => {
    if (!cancelled && activePane()?.id === paneId) toggle();
  }).catch((error) => {
    console.error("[oculus] conversations shortcut unavailable", error);
    return () => {};
  });
  return () => {
    // A native event can arrive before registration resolves or unlisten
    // reaches Rust. Unmounted panes must ignore both windows.
    cancelled = true;
    void subscription.then((off) => off()).catch(() => {});
  };
}

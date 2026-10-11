import { browser, browseId } from "@/lib/browser";
import { ownsPlayback, stopLecturePlayback } from "@/lib/lectures/playback";
import { dropTabRouter } from "@/lib/shell/tabRouters";
import type { SidePanel } from "@/lib/shell/sideStack";
import type { PaneState } from "./panes";

/** A side item that is going away: its router is dropped and, unless its page
 *  is handed over to another pane, its lecture stops and its browser page
 *  closes — left open, `useBrowserTabs` would adopt it as a new strip tab. */
export function releaseItem(item: PaneState, handover = false): void {
  dropTabRouter(item.id);
  if (handover) return;
  if (ownsPlayback(item.id)) stopLecturePlayback();
  const page = browseId(item.path);
  if (page != null) browser.close(page).catch(() => {});
}

/** A lecture sent to the back of the side panel pauses: only the front item
 *  is mounted to show it. */
export function leaveFront(side: SidePanel, next: SidePanel | null): void {
  const was = side.front;
  if (next?.front !== was && ownsPlayback(was)) stopLecturePlayback();
}

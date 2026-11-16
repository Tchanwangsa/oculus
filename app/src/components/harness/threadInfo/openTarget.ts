import { openCitation } from "@/lib/files/openFile";
import type { InfoTarget } from "@/lib/harness/threadInfo";
import { openBeside } from "@/lib/shell/tabRouters";
import { useTabStore } from "@/stores/shell/tabStore";

/** Opens a row's thing as a file chip would: beside the chat, or with
 *  `newTab` (⌘-click, middle click) in a tab of its own. */
export function openTarget(target: InfoTarget, newTab: boolean): void {
  if (target.kind === "library") {
    openCitation({ path: target.path, page: target.page, at: target.at }, newTab);
    return;
  }
  if (newTab) useTabStore.getState().addTab(target.href);
  else openBeside(target.href);
}

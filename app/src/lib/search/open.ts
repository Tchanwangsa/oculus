import { openFileSmart } from "@/lib/files/openFile";
import type { SearchItem } from "./types";

/** Injected per surface: the palette navigates the shell from outside every
 *  router, the new-tab field the pane it is drawn in. */
export interface OpenSearchOptions {
  /** ⌘-click / ⌘↵ — somewhere new, rather than here. */
  newTab: boolean;
  navigate: (path: string) => void;
  addTab: (path: string) => void;
  openUrl: (url: string) => void;
}

export function openSearchItem(item: SearchItem, o: OpenSearchOptions): void {
  switch (item.target.kind) {
    case "file":
      // System viewer; ⌘ changes nothing.
      openFileSmart(item.target.file);
      return;
    case "url":
      o.openUrl(item.target.url);
      return;
    case "route":
      (o.newTab ? o.addTab : o.navigate)(item.target.path);
      return;
    case "filter-key":
    case "filter":
      // The palette edits its own field for these before it gets here.
      return;
  }
}

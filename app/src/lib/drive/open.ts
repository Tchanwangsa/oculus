import { invoke } from "@tauri-apps/api/core";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { filePageHref, openFileSmart } from "@/lib/files/openFile";
import { lecturePagePath } from "@/lib/lectures/pagePath";
import { navigateActive } from "@/lib/shell/tabRouters";
import { useTabStore } from "@/stores/shell/tabStore";
import { loadDataDir } from "@/hooks/backend/useDataDir";
import type { DriveItem } from "./items";
import { reportDriveProblem } from "./actions";

/** Where an entry's page is, for `data-tab-href` (⌘-click) and a new tab;
 *  null for a file only the system viewer opens. */
export function itemHref(item: DriveItem, folderHref: (rel: string) => string): string | null {
  switch (item.type) {
    case "folder": return folderHref(item.key);
    case "recording": return lecturePagePath({ id: item.entry.lectureId, subject_id: item.entry.subjectId, title: item.entry.title });
    case "file": return item.row ? filePageHref(item.row) : null;
  }
}

/** Opens an entry as the app opens it anywhere else: a folder here, a
 *  recording's lecture page, a file beside the page (`openFileSmart`); a file
 *  with no row yet in the system viewer. */
export function openItem(item: DriveItem, folderHref: (rel: string) => string): void {
  if (item.type === "file") {
    if (item.row) openFileSmart(item.row);
    else invoke("open_course_file", { relativePath: item.key }).catch((e) => reportDriveProblem(String(e)));
    return;
  }
  const href = itemHref(item, folderHref);
  if (href) navigateActive(href);
}

export function openInNewTab(item: DriveItem, folderHref: (rel: string) => string): void {
  const href = itemHref(item, folderHref);
  if (href) useTabStore.getState().addTab(href);
}

export async function revealItem(item: DriveItem): Promise<void> {
  const dataDir = await loadDataDir();
  if (!dataDir) return;
  await revealItemInDir(`${dataDir}/${item.key}`).catch((e) => reportDriveProblem(String(e)));
}

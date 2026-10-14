import type { VersionKind } from "@/lib/notes/documentVersions";

export const PANEL = {
  defaultWidth: 320,
  minWidth: 260,
  maxWidth: 640,
  side: "right",
  // Hiding is the header's button or the bar's Versions toggle, never a drag.
  collapseThreshold: 0,
  storageKey: "oculus-document-history",
} as const;

/** The most of History's width the sidebar may take, so the page keeps room. */
export const PANEL_MAX_SHARE = "50%";

/** What a snapshot row says when it has no label of its own. */
export const SNAPSHOT_HINT: Record<Exclude<VersionKind, "checkpoint">, string> = {
  auto: "Autosave snapshot",
  external: "Changed outside the editor",
  restore: "Before a restore",
};

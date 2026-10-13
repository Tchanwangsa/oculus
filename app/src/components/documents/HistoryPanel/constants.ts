import type { VersionKind } from "@/lib/notes/documentVersions";

export const PANEL = {
  defaultWidth: 320,
  minWidth: 260,
  maxWidth: 640,
  side: "right",
  // Closing is the header's toggle, so a drag never folds it.
  collapseThreshold: 0,
  storageKey: "oculus-document-history",
} as const;

/** Editor-root widths (CSS px) where the list floats over the page, then docks. */
export const HISTORY_TIERS = { medium: 560, wide: 880 } as const;

/** What a snapshot row says when it has no label of its own. */
export const SNAPSHOT_HINT: Record<Exclude<VersionKind, "checkpoint">, string> = {
  auto: "Autosave snapshot",
  external: "Changed outside the editor",
  restore: "Before a restore",
};

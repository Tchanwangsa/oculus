import type { CSSProperties } from "react";

export type StageKey = "download" | "parse" | "embed";

export const STAGE_LABEL: Record<StageKey, string> = {
  download: "Download",
  parse: "Parse",
  embed: "Embed",
};

/** Shared by header and rows. The time column also holds the hover actions
 *  (three `icon-xs` buttons). Sized by the table's own width (`@container`):
 *  narrow, the caption folds into the track's tooltip and the name keeps the
 *  room. */
export const COLS =
  "grid grid-cols-[minmax(0,1fr)_minmax(180px,260px)_80px] @max-2xl:grid-cols-[minmax(0,1fr)_96px_80px] items-center gap-4 @max-2xl:gap-3 px-5";

/** Diagonal stripes: a stage the user skipped, distinct from one not reached. */
export const HATCH: CSSProperties = {
  backgroundImage:
    "repeating-linear-gradient(-45deg, color-mix(in srgb, var(--color-muted-foreground) 45%, transparent) 0 1.5px, transparent 1.5px 4px)",
};

export const SKIP_HINT = "Skip — stops this file's parse. You can parse it later.";

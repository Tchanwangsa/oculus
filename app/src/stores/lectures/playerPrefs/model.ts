import type { SourceNum } from "@/lib/db";

/** Which edge of the player the docked panel is attached to. */
export type Dock = "bottom" | "top" | "left" | "right";

export type DockTab = "chapters" | "transcript" | "chat";

/** Every valid tab; the tolerant reads below check against this one list. */
export const DOCK_TABS: DockTab[] = ["chapters", "transcript", "chat"];

/** Restore a stored tab order tolerantly: unknown and duplicate entries are
 *  dropped, missing tabs appended. */
export function orderDockTabs(stored: unknown): DockTab[] {
  const out: DockTab[] = [];
  if (Array.isArray(stored)) {
    for (const v of stored) {
      if (DOCK_TABS.includes(v as DockTab) && !out.includes(v as DockTab)) {
        out.push(v as DockTab);
      }
    }
  }
  for (const t of DOCK_TABS) if (!out.includes(t)) out.push(t);
  return out;
}

/** Fold a reorder of the *visible* tabs back into the full order, leaving a
 *  hidden tab (Transcript, on a lecture without one) in its slot. */
export function reorderDockTabs(full: DockTab[], visibleNext: DockTab[]): DockTab[] {
  const visible = new Set(visibleNext);
  let i = 0;
  return full.map((t) => (visible.has(t) ? visibleNext[i++] : t));
}

export const isVertical = (dock: Dock) => dock === "bottom" || dock === "top";

export const SPEED_MIN = 0.25;
export const SPEED_MAX = 2;
export const SPEED_STEP = 0.05;

export const SPEED_PRESETS = [0.5, 1, 1.25, 1.5, 1.75, 2] as const;

/** Snap to a step; `toFixed` strips float dust that would break a preset's `===`. */
export const clampSpeed = (s: number) =>
  Number(
    Math.min(
      SPEED_MAX,
      Math.max(SPEED_MIN, Math.round(s / SPEED_STEP) * SPEED_STEP),
    ).toFixed(2),
  );

/** Volume is a 0–1 fraction end to end, as `video.volume` takes it. */
export const clampVolume = (v: number) =>
  Number(Math.min(1, Math.max(0, v)).toFixed(2));

const DEFAULT_H = 176;
const DEFAULT_W = 320;

export type Layout = "single" | "pip" | "stack";

/** PIP width as a fraction of the video area. Height follows the aspect. */
export const PIP_MIN_W = 0.12;
export const PIP_MAX_W = 0.6;

/** Pixel floor under `PIP_MIN_W`, applied in `useSourceLayout`. */
export const PIP_MIN_PX_W = 160;
export const PIP_MIN_PX_H = 90;

const SPLIT_MIN = 0.15;
const SPLIT_MAX = 0.85;

export const clampPipWidth = (w: number) =>
  Math.min(PIP_MAX_W, Math.max(PIP_MIN_W, w));

export const clampSplit = (v: number) =>
  Math.min(SPLIT_MAX, Math.max(SPLIT_MIN, v));

/** Keep a fraction inside 0…1 given something of size `size` sits at it. */
export const clampOffset = (v: number, size: number) =>
  Math.min(Math.max(0, 1 - size), Math.max(0, v));

/** Player habits across lectures. Per-lecture position lives in the DB
 *  (`lectures.progress_seconds`). */
export interface PlayerPrefs {
  dock: Dock;
  dockTab: DockTab;
  dockTabOrder: DockTab[];
  height: number;
  width: number;
  speed: number;
  volume: number;
  muted: boolean;
  captionsEnabled: boolean;
  transcriptVisible: boolean;
  layout: Layout;
  /** The stream in the main frame (the big one in `pip`, the top in `stack`). */
  mainSource: SourceNum;
  /** PIP box as fractions of the video area; height follows the aspect. */
  pipX: number;
  pipY: number;
  pipW: number;
  /** Share of the stacked view's height the top screen takes. */
  split: number;
}

export const DEFAULTS: PlayerPrefs = {
  dock: "bottom",
  // Every downloaded lecture has a transcript; chapters must be asked for.
  dockTab: "transcript",
  dockTabOrder: [...DOCK_TABS],
  height: DEFAULT_H,
  width: DEFAULT_W,
  speed: 1,
  volume: 1,
  muted: false,
  captionsEnabled: false,
  transcriptVisible: true,
  layout: "single",
  mainSource: 1,
  pipX: 0.72,
  pipY: 0.62,
  pipW: 0.26,
  split: 0.68,
};

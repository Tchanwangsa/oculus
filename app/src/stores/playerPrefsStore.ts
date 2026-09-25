import { create } from "zustand";
import type { SourceNum } from "@/lib/db";

/** Which edge of the player the docked panel is attached to. */
export type Dock = "bottom" | "top" | "left" | "right";

export type DockTab = "chapters" | "transcript" | "chat";

/**
 * The Transcript tab's register: `standard` is the VTT cue list, `enhanced` the
 * reading copy (`docs/chapters.md`). A stored `enhanced` falls back to
 * `standard` on a lecture without one (`modeInFront`).
 */
export type TranscriptMode = "standard" | "enhanced";

const TRANSCRIPT_MODES: TranscriptMode[] = ["standard", "enhanced"];

/** Every valid tab; the tolerant reads below check against this one list. */
const DOCK_TABS: DockTab[] = ["chapters", "transcript", "chat"];

/** Restore a stored tab order tolerantly: unknown and duplicate entries are
 *  dropped, missing tabs appended. */
function orderDockTabs(stored: unknown): DockTab[] {
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

// ── Two sources ──────────────────────────────────────────────────────────────

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
  transcriptMode: TranscriptMode;
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

const DEFAULTS: PlayerPrefs = {
  dock: "bottom",
  // Every downloaded lecture has a transcript; chapters must be asked for.
  dockTab: "transcript",
  dockTabOrder: [...DOCK_TABS],
  transcriptMode: "standard",
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

const KEY = "oculus-lecture-player-prefs";

/** Read tolerantly: a missing or malformed key falls back to its default. */
function load(): PlayerPrefs {
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) {
      // Write before clearing the legacy keys, so a crash between loses nothing.
      const migrated = { ...DEFAULTS, ...loadLegacy() };
      try {
        localStorage.setItem(KEY, JSON.stringify(migrated));
        clearLegacy();
      } catch {
        /* ignore */
      }
      return migrated;
    }
    const p = JSON.parse(raw) as Partial<PlayerPrefs>;
    return {
      dock:
        p.dock === "bottom" || p.dock === "top" || p.dock === "left" || p.dock === "right"
          ? p.dock
          : DEFAULTS.dock,
      dockTab: DOCK_TABS.includes(p.dockTab as DockTab) ? (p.dockTab as DockTab) : DEFAULTS.dockTab,
      dockTabOrder: orderDockTabs(p.dockTabOrder),
      transcriptMode: TRANSCRIPT_MODES.includes(p.transcriptMode as TranscriptMode)
        ? (p.transcriptMode as TranscriptMode)
        : DEFAULTS.transcriptMode,
      height: num(p.height, DEFAULTS.height),
      width: num(p.width, DEFAULTS.width),
      speed:
        typeof p.speed === "number" && Number.isFinite(p.speed)
          ? clampSpeed(p.speed)
          : DEFAULTS.speed,
      volume:
        typeof p.volume === "number" && Number.isFinite(p.volume)
          ? clampVolume(p.volume)
          : DEFAULTS.volume,
      muted: !!p.muted,
      captionsEnabled: !!p.captionsEnabled,
      transcriptVisible: p.transcriptVisible !== false,
      layout:
        p.layout === "single" || p.layout === "pip" || p.layout === "stack"
          ? p.layout
          : DEFAULTS.layout,
      mainSource: p.mainSource === 2 ? 2 : 1,
      // `frac` allows 0 (a flush-left PIP); `num` does not.
      pipX: clampOffset(frac(p.pipX, DEFAULTS.pipX), frac(p.pipW, DEFAULTS.pipW)),
      pipY: frac(p.pipY, DEFAULTS.pipY),
      pipW: clampPipWidth(frac(p.pipW, DEFAULTS.pipW)),
      split: clampSplit(frac(p.split, DEFAULTS.split)),
    };
  } catch {
    return DEFAULTS;
  }
}

const num = (v: unknown, fallback: number) =>
  typeof v === "number" && Number.isFinite(v) && v > 0 ? v : fallback;

const frac = (v: unknown, fallback: number) =>
  typeof v === "number" && Number.isFinite(v) && v >= 0 && v <= 1 ? v : fallback;

/** Loose pre-store keys, migrated once into `KEY`. */
const LEGACY = [
  "oculus-lecture-transcript-dock",
  "oculus-lecture-transcript-height",
  "oculus-lecture-transcript-width",
] as const;

function loadLegacy(): Partial<PlayerPrefs> {
  const out: Partial<PlayerPrefs> = {};
  try {
    const d = localStorage.getItem(LEGACY[0]);
    if (d === "bottom" || d === "top" || d === "left" || d === "right") out.dock = d;
    const h = Number(localStorage.getItem(LEGACY[1]));
    if (Number.isFinite(h) && h > 0) out.height = h;
    const w = Number(localStorage.getItem(LEGACY[2]));
    if (Number.isFinite(w) && w > 0) out.width = w;
  } catch {
    /* ignore */
  }
  return out;
}

function clearLegacy() {
  for (const k of LEGACY) localStorage.removeItem(k);
}

// Debounced: a resize drag writes every frame and `setItem` is synchronous.
let saveTimer: ReturnType<typeof setTimeout> | null = null;
function save(prefs: PlayerPrefs) {
  if (saveTimer) clearTimeout(saveTimer);
  saveTimer = setTimeout(() => {
    try {
      localStorage.setItem(KEY, JSON.stringify(prefs));
    } catch {
      /* ignore */
    }
  }, 200);
}

interface PlayerPrefsState extends PlayerPrefs {
  set: (patch: Partial<PlayerPrefs>) => void;
}

export const usePlayerPrefs = create<PlayerPrefsState>((set, get) => ({
  ...load(),
  set: (patch) => {
    set(patch);
    const { set: _, ...prefs } = get();
    save(prefs);
  },
}));

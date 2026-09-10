import { create } from "zustand";

/** Which edge of the player the transcript panel is attached to. */
export type Dock = "bottom" | "top" | "left" | "right";

export const isVertical = (dock: Dock) => dock === "bottom" || dock === "top";

/** Playback speed is continuous in 0.05 steps, like YouTube's. */
export const SPEED_MIN = 0.25;
export const SPEED_MAX = 2;
export const SPEED_STEP = 0.05;

/** The speeds worth one tap; the slider reaches everything between them. */
export const SPEED_PRESETS = [0.5, 1, 1.25, 1.5, 1.75, 2] as const;

/**
 * Snap to the nearest step inside the range. Both a 0.05-stepped slider and
 * repeated `+ 0.05` accumulate binary-float dust (1.7500000000000002), which
 * would otherwise reach the badge and the `===` that highlights a preset.
 */
export const clampSpeed = (s: number) =>
  Number(
    Math.min(
      SPEED_MAX,
      Math.max(SPEED_MIN, Math.round(s / SPEED_STEP) * SPEED_STEP),
    ).toFixed(2),
  );

/**
 * Volume is a fraction, and the element takes it as one — no scaling anywhere
 * between the slider and `video.volume`, which is where percent/fraction
 * mix-ups live.
 */
export const clampVolume = (v: number) =>
  Number(Math.min(1, Math.max(0, v)).toFixed(2));

export const DEFAULT_H = 176;
export const DEFAULT_W = 320;

/**
 * How *this person* likes the lecture player, not how one lecture was left.
 * Speed, captions and the transcript's side and size are habits — you pick
 * 1.5× and a left-docked transcript once and every recording opens that way.
 * The per-lecture state is playback position, and that lives in the DB
 * (`lectures.progress_seconds`), not here.
 */
export interface PlayerPrefs {
  dock: Dock;
  height: number;
  width: number;
  speed: number;
  /** 0–1, the element's own scale. Kept apart from `muted` so unmuting
      returns to the level you were listening at, not to full. */
  volume: number;
  muted: boolean;
  captionsEnabled: boolean;
  transcriptVisible: boolean;
}

const DEFAULTS: PlayerPrefs = {
  dock: "bottom",
  height: DEFAULT_H,
  width: DEFAULT_W,
  speed: 1,
  volume: 1,
  muted: false,
  captionsEnabled: false,
  transcriptVisible: true,
};

const KEY = "oculus-lecture-player-prefs";

/**
 * Read tolerantly: this is user state that outlives any one version of the
 * app, so a key that has gone missing or gone weird falls back to its default
 * rather than taking the whole player down.
 */
function load(): PlayerPrefs {
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) {
      // Persist the migrated values *before* dropping the old keys. Removing
      // them first loses the layout outright if this load never gets as far as
      // a write — which is exactly what a crash between the two would do.
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
    };
  } catch {
    return DEFAULTS;
  }
}

const num = (v: unknown, fallback: number) =>
  typeof v === "number" && Number.isFinite(v) && v > 0 ? v : fallback;

/**
 * The dock side and size used to live in three loose keys of their own. Read
 * them once so an existing layout survives the move into this store.
 */
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

// Debounced: a resize drag changes `height`/`width` every frame, and
// `localStorage.setItem` is synchronous — writing on every frame is enough to
// show up as stutter in the drag itself.
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

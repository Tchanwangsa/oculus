import { create } from "zustand";
import type { SourceNum } from "@/lib/db";

/** Which edge of the player the docked panel is attached to. */
export type Dock = "bottom" | "top" | "left" | "right";

/** Which of the dock's three readings of the recording is in front. Chat is
 *  the odd one: the other two are the recording read back — as its shape, and
 *  as its words — and it is a conversation about it; but it is the same dock
 *  and the same preference. */
export type DockTab = "chapters" | "transcript" | "chat";

/**
 * Which register the Transcript tab is showing.
 *
 * `standard` is the cue list — every three-second fragment of what the
 * recogniser heard. `enhanced` is the same recording through the reading
 * copy: the same words with the filler dropped, the mis-heard notation fixed
 * off the slide and the spoken maths set as maths (`docs/chapters.md`). They
 * are one tab and not two because the second is the first made readable, and
 * the transcript is what you are looking for either way.
 *
 * A habit like the dock's side: someone who reads the enhanced copy reads it
 * on every lecture. It defaults to `standard`, which every downloaded
 * recording has — the enhanced copy is a job that has to be asked for, and a
 * stored `enhanced` falls back to it per lecture until there is one
 * (`modeInFront`).
 */
export type TranscriptMode = "standard" | "enhanced";

const TRANSCRIPT_MODES: TranscriptMode[] = ["standard", "enhanced"];

/** Every value the tab may hold, so the tolerant read below stays one list.
 *  It hard-coded `=== "chapters"` once, which silently reset a stored `chat`
 *  to the default on the next launch. Two names have passed through this list
 *  and are gone: `recap`, and the `read` tab that briefly held the chapter
 *  list and the reading copy together. Neither is in it now, so a stored
 *  value naming either falls to the default through that same tolerant read,
 *  and `orderDockTabs` drops it from a stored order. */
const DOCK_TABS: DockTab[] = ["chapters", "transcript", "chat"];

/**
 * Put the dock's tabs back in a stored order, tolerantly.
 *
 * The order is user state that outlives any one version of the app, so this
 * has to survive a stored value that repeats a tab, names one that no longer
 * exists, or is missing one that has since been added — a saved order from
 * before Chat shipped is one case, and one that still names the two tabs
 * that became Read is the other. Unknown and duplicate entries
 * are dropped and anything missing is appended in its default position, which
 * means a new tab shows up at the end rather than silently not at all.
 */
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

/**
 * Fold a reordering of the *visible* tabs back into the full stored order.
 *
 * The strip drops the Transcript tab on a recording that has none on disk, so
 * the list the drag rearranged is not always the list that is stored. Walking
 * the full order and refilling only the slots that held a visible tab keeps the
 * hidden one exactly where it was — otherwise a reorder done on a lecture with
 * no transcript would quietly shunt Transcript to the end for every lecture.
 */
export function reorderDockTabs(full: DockTab[], visibleNext: DockTab[]): DockTab[] {
  const visible = new Set(visibleNext);
  let i = 0;
  return full.map((t) => (visible.has(t) ? visibleNext[i++] : t));
}

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

// ── Two sources ──────────────────────────────────────────────────────────────

/** How the two streams of a capture share the frame. */
export type Layout = "single" | "pip" | "stack";

/** Which stream a frame shows — the row's own type, so the two never drift. */
export type { SourceNum };

/** PIP width as a fraction of the video area. Height follows the aspect. */
export const PIP_MIN_W = 0.12;
export const PIP_MAX_W = 0.6;

/**
 * …and a floor in pixels underneath that, because a fraction is not a size.
 * The same 12% is a postage stamp in a peek panel and a comfortable inset
 * fullscreen, and the small end is where the camera stops being watchable.
 * Applied in `useSourceLayout`, which is where the area is measured; the
 * height floor reaches the width through the picture's locked aspect.
 */
export const PIP_MIN_PX_W = 160;
export const PIP_MIN_PX_H = 90;

/** How little of the stacked view either screen can be squeezed to. */
export const SPLIT_MIN = 0.15;
export const SPLIT_MAX = 0.85;

export const clampPipWidth = (w: number) =>
  Math.min(PIP_MAX_W, Math.max(PIP_MIN_W, w));

export const clampSplit = (v: number) =>
  Math.min(SPLIT_MAX, Math.max(SPLIT_MIN, v));

/** Keep a fraction inside 0…1 given something of size `size` sits at it. */
export const clampOffset = (v: number, size: number) =>
  Math.min(Math.max(0, 1 - size), Math.max(0, v));

/**
 * How *this person* likes the lecture player, not how one lecture was left.
 * Speed, captions and the transcript's side and size are habits — you pick
 * 1.5× and a left-docked transcript once and every recording opens that way.
 * The per-lecture state is playback position, and that lives in the DB
 * (`lectures.progress_seconds`), not here.
 */
export interface PlayerPrefs {
  dock: Dock;
  /** The tab the dock opens on. A habit like the side it is docked to, not a
   *  property of one recording: someone who reads chapters reads them for
   *  every lecture, and re-picking the tab per lecture is the friction. */
  dockTab: DockTab;
  /** What order the three sit in, left to right — dragged by hand in the dock's
   *  header. A habit like the side and the tab, and the same argument: the tab
   *  you reach for first is the same one on every recording. */
  dockTabOrder: DockTab[];
  /** Which register the Transcript tab is in — see `TranscriptMode`. */
  transcriptMode: TranscriptMode;
  height: number;
  width: number;
  speed: number;
  /** 0–1, the element's own scale. Kept apart from `muted` so unmuting
      returns to the level you were listening at, not to full. */
  volume: number;
  muted: boolean;
  captionsEnabled: boolean;
  transcriptVisible: boolean;
  /** How the two sources share the frame, when a lecture has two. */
  layout: Layout;
  /** The stream in the main frame — the only one in `single`, the big one in
      `pip`, the top one in `stack`. The other frame takes the other stream. */
  mainSource: SourceNum;
  /** PIP box, in fractions of the video area. Its height is not stored: it
      follows the picture's own aspect, which is what locks the ratio. */
  pipX: number;
  pipY: number;
  pipW: number;
  /** Share of the stacked view's height the top screen takes. */
  split: number;
}

const DEFAULTS: PlayerPrefs = {
  dock: "bottom",
  // Transcript: every downloaded lecture has one, and chapters are a job that
  // has to be asked for. Opening on a tab that is usually empty is worse.
  dockTab: "transcript",
  dockTabOrder: [...DOCK_TABS],
  // …and standard inside it, for the same reason one tab over.
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
  // Bottom-right, out of the way of slide content and clear of the scrub bar.
  pipX: 0.72,
  pipY: 0.62,
  pipW: 0.26,
  // The Presenter screen is the one you read; the camera only has to be legible.
  split: 0.68,
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
      // `frac` allows 0, which `num` does not — a PIP flush to the left edge
      // is a real position, and a width of 0 is not.
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

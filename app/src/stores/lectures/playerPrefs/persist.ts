import {
  DEFAULTS,
  DOCK_TABS,
  clampOffset,
  clampPipWidth,
  clampSpeed,
  clampSplit,
  clampVolume,
  orderDockTabs,
  type DockTab,
  type PlayerPrefs,
} from "./model";

const KEY = "oculus-lecture-player-prefs";

/** Read tolerantly: a missing or malformed key falls back to its default. */
export function load(): PlayerPrefs {
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
export function save(prefs: PlayerPrefs) {
  if (saveTimer) clearTimeout(saveTimer);
  saveTimer = setTimeout(() => {
    try {
      localStorage.setItem(KEY, JSON.stringify(prefs));
    } catch {
      /* ignore */
    }
  }, 200);
}


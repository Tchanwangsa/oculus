import type { Find } from "@/components/files/pdf/types";

export const MODE_KEY = "oculus-pdf-layout";

/** pdf.js's own clamps (`MIN_SCALE`/`MAX_SCALE`), mirrored only to grey out
 *  the toolbar buttons. Scale is absolute (1 = actual size), not relative to
 *  the fit, so a slide fitted to the side panel already sits near 0.37. */
export const MIN_ZOOM = 0.1;
export const MAX_ZOOM = 10;

/** One toolbar press, as a ratio — pdf.js's `steps: 1` rounds to a tenth,
 *  which stutters at small scales. */
export const ZOOM_STEP = 1.1;

/** `updateScale`'s `drawingDelay`: pdf.js previews via `--scale-factor` at
 *  once and re-rasterises this long after the last change (>= 1000 disables
 *  the postponement). */
export const DRAW_DELAY = 400;

/** Pixels per line when `deltaMode === DOM_DELTA_LINE`. */
export const LINE_HEIGHT = 16;

/** WebKit sometimes drops `gestureend`, which would latch `gestureActive` and
 *  kill ⌘-scroll zoom; the flag releases itself after this much quiet. */
export const GESTURE_LAPSE = 400;

/** Scale presets that are re-applied on a container resize; a number the
 *  reader chose survives it. */
export const FIT_VALUES = new Set(["auto", "page-width", "page-fit", "page-actual"]);

/** `auto` is page-width capped at 125%; plain `page-width` blows a 16:9 slide
 *  past the screen in the full-page view. */
export const DEFAULT_FIT = "auto";

/** The class a cited passage's text-layer spans carry (`index.css`). */
export const HIT = "citation-hit";

/** In continuous scroll, a page counts toward the toolbar's range when it
 *  fills this share of the viewport — a sliver at an edge is not being read —
 *  or shows this share of itself, which catches small pages when zoomed out. */
export const VIEWPORT_SHARE = 0.2;
export const PAGE_SHARE = 0.5;

export const FIND_CLOSED: Find = { open: false, query: "" };

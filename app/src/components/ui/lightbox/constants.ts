export const MIN_ZOOM = 0.1;
export const MAX_ZOOM = 8;
export const STEP = 1.25;

/** Cap on the initial fit, so a small diagram fills the window without
 *  looking blown up. */
export const MAX_FIT = 2.5;

/** Gutter at fit, in px: `p-9` each side; the bottom also clears the
 *  floating toolbar (`pb-20`). */
export const GUTTER_X = 72;
export const GUTTER_Y = 36 + 80;

/**
 * Time constant of the zoom smoother, in ms. Inputs arrive in steps (a wheel
 * notch, a quantised pinch `scale`, a button's 1.25×), so they only move a
 * target and the painted zoom eases towards it every frame.
 */
export const SMOOTH_MS = 45;

/** Zoom per pixel of ⌘-scroll, as an exponent (zoom is multiplicative). A
 *  macOS wheel notch is ±120px, so this makes one notch ~1.3×. */
export const WHEEL_GAIN = 0.0022;

/** Per-event cap, so trackpad momentum can't cross the range in a frame. */
export const WHEEL_MAX_STEP = 1.3;

/** `gestureend` can go undelivered (pinch ends off-window), so the pinch is
 *  assumed over this long after its last event. */
export const GESTURE_IDLE_MS = 400;

export const clampZoom = (z: number) => Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, z));

/** The content's natural, unscaled size — all viewer maths is in it. */
export type LightboxSize = { width: number; height: number };

/** Which clamp the zoom is at, for greying a button — the only React state a
 *  zoom touches. */
export type Limit = "none" | "min" | "max";

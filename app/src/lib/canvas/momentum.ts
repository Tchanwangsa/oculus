import type { CanvasViewport } from './model';

/** Camera speed in screen pixels per millisecond. */
export interface Velocity { x: number; y: number }

/** The release speed is read from this many milliseconds of samples before the release. */
export const VELOCITY_WINDOW_MS = 100;
/** A finger that has not moved for this long before lifting was held still: no fling. */
export const STILL_MS = 50;
/** Release speeds are capped here so one noisy sample cannot throw the page thousands of pixels. */
export const MAX_SPEED = 6;
/** The samples a speed is read from must span this long; a burst of late-delivered moves spans almost nothing. */
export const MIN_SPAN_MS = 30;
/** A pan that has moved the camera less than this in all is a tap or a resting hand, never a flick. */
export const MIN_TRAVEL = 24;
/** Slower releases are a placed finger, not a flick. */
export const MIN_SPEED = 0.25;
/** A glide ends below this speed. */
export const STOP_SPEED = 0.02;
/** UIScrollView's normal deceleration: speed is multiplied by this every millisecond. */
export const DECELERATION = 0.998;
/**
 * A larger net zoom change across the window is a pinch, whose camera x/y follow the zoom rather than a flick. Each
 * finger moves in its own event, so the zoom wobbles between them; only the change from first to last sample counts.
 */
const PINCH_RATIO = 1.05;
/** Two fingers that zoomed this much over the whole gesture were pinching, whatever the last moment looked like. */
const GESTURE_ZOOM_RATIO = 1.1;

type Sample = { time: number; x: number; y: number; zoom: number };

const speed = (velocity: Velocity) => Math.hypot(velocity.x, velocity.y);

/** Why a release did or didn't glide, for the companion's diagnostics. */
export type ReleaseVerdict = 'glide' | 'few-samples' | 'held' | 'short-span' | 'short-travel' | 'pinch' | 'slow';
export interface ReleaseTrace { verdict: ReleaseVerdict; travel: number; spanMs: number; samples: string }

/** The camera samples of one pan, from which its release speed is read. */
export class VelocityTracker {
  private samples: Sample[] = [];
  private origin: Sample | null = null;
  /** The whole pan, kept only for `lastRelease`'s trace. */
  private history: Sample[] = [];
  lastRelease: ReleaseTrace | null = null;

  /** Starts a pan at `viewport`. */
  start(time: number, viewport: CanvasViewport) { this.samples = []; this.history = []; this.add(time, viewport); this.origin = this.samples[0]; }

  /** Records the camera at `time`; an unmoved camera adds nothing, so a held finger leaves its last sample to go stale. */
  add(time: number, { x, y, zoom }: CanvasViewport) {
    const last = this.samples[this.samples.length - 1];
    if (last && last.x === x && last.y === y && last.zoom === zoom) return;
    this.samples.push({ time, x, y, zoom });
    if (this.history.length < 240) this.history.push({ time, x, y, zoom });
    while (this.samples.length > 2 && this.samples[0].time < time - VELOCITY_WINDOW_MS) this.samples.shift();
  }

  /** The speed to glide at after a release at `time`, capped at `MAX_SPEED`; null for a held finger, a pinch or a slow release. */
  release(time: number): Velocity | null {
    const recent = this.samples.filter(sample => sample.time >= time - VELOCITY_WINDOW_MS), origin = this.origin;
    const first = recent[0], last = recent[recent.length - 1];
    const travel = origin && last ? Math.hypot(last.x - origin.x, last.y - origin.y) : 0;
    const verdict = (): [ReleaseVerdict, Velocity | null] => {
      if (recent.length < 2) return ['few-samples', null];
      if (time - last.time > STILL_MS) return ['held', null];
      if (last.time - first.time < MIN_SPAN_MS) return ['short-span', null];
      if (travel < MIN_TRAVEL) return ['short-travel', null];
      if (Math.max(last.zoom / first.zoom, first.zoom / last.zoom) > PINCH_RATIO) return ['pinch', null];
      if (origin && Math.max(last.zoom / origin.zoom, origin.zoom / last.zoom) > GESTURE_ZOOM_RATIO) return ['pinch', null];
      const elapsed = last.time - first.time;
      const velocity = { x: (last.x - first.x) / elapsed, y: (last.y - first.y) / elapsed };
      const measured = speed(velocity);
      if (measured < MIN_SPEED) return ['slow', null];
      const scale = Math.min(1, MAX_SPEED / measured);
      return ['glide', { x: velocity.x * scale, y: velocity.y * scale }];
    };
    const [outcome, velocity] = verdict();
    const base = this.history[0];
    this.lastRelease = {
      verdict: outcome, travel: Math.round(travel), spanMs: first && last ? Math.round(last.time - first.time) : 0,
      // "ms,dx,dy" from the pan's start, the last 80 samples, then the release.
      samples: base ? [...this.history.slice(-80).map(s => `${Math.round(s.time - base.time)},${Math.round(s.x - base.x)},${Math.round(s.y - base.y)}`), `up@${Math.round(time - base.time)}`].join(' ') : '',
    };
    this.samples = []; this.origin = null; this.history = [];
    return velocity;
  }
}

/**
 * When an input event happened, on `performance.now()`'s clock. Samples are timed by the event, not by when the page
 * got round to handling it: moves queued behind a slow frame arrive together and would otherwise read as a violent flick.
 */
export function eventTime(event: { timeStamp?: number }, now = performance.now()): number {
  const stamp = event.timeStamp;
  return typeof stamp === 'number' && Number.isFinite(stamp) && stamp > 0 && stamp <= now + 5 && now - stamp < 1000 ? stamp : now;
}

/**
 * Advances a glide by `elapsed` milliseconds of exponential deceleration: speed v·rᵗ, so the distance is the exact
 * integral v·(rᵗ − 1)/ln r and the path is the same at any frame rate.
 */
export function glide(velocity: Velocity, elapsed: number): { dx: number; dy: number; velocity: Velocity } {
  const decay = DECELERATION ** elapsed, distance = (decay - 1) / Math.log(DECELERATION);
  return { dx: velocity.x * distance, dy: velocity.y * distance, velocity: { x: velocity.x * decay, y: velocity.y * decay } };
}

/** Whether a glide at `velocity` is still moving. */
export const gliding = (velocity: Velocity) => speed(velocity) >= STOP_SPEED;

import type { CanvasViewport } from './model';

/** Camera speed in screen pixels per millisecond. */
export interface Velocity { x: number; y: number }

/** The release speed is read from this many milliseconds of samples before the release. */
export const VELOCITY_WINDOW_MS = 100;
/** A finger that has not moved for this long before lifting was held still: no fling. */
export const STILL_MS = 50;
/** Release speeds are capped here so one noisy sample cannot throw the page thousands of pixels. */
export const MAX_SPEED = 8;
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

type Sample = { time: number; x: number; y: number; zoom: number };

const speed = (velocity: Velocity) => Math.hypot(velocity.x, velocity.y);

/** The camera samples of one pan, from which its release speed is read. */
export class VelocityTracker {
  private samples: Sample[] = [];

  /** Starts a pan at `viewport`. */
  start(time: number, viewport: CanvasViewport) { this.samples = []; this.add(time, viewport); }

  /** Records the camera at `time`; an unmoved camera adds nothing, so a held finger leaves its last sample to go stale. */
  add(time: number, { x, y, zoom }: CanvasViewport) {
    const last = this.samples[this.samples.length - 1];
    if (last && last.x === x && last.y === y && last.zoom === zoom) return;
    this.samples.push({ time, x, y, zoom });
    while (this.samples.length > 2 && this.samples[0].time < time - VELOCITY_WINDOW_MS) this.samples.shift();
  }

  /** The speed to glide at after a release at `time`, capped at `MAX_SPEED`; null for a held finger, a pinch or a slow release. */
  release(time: number): Velocity | null {
    const recent = this.samples.filter(sample => sample.time >= time - VELOCITY_WINDOW_MS);
    this.samples = [];
    const first = recent[0], last = recent[recent.length - 1];
    if (recent.length < 2 || time - last.time > STILL_MS || last.time <= first.time) return null;
    if (Math.max(last.zoom / first.zoom, first.zoom / last.zoom) > PINCH_RATIO) return null;
    const elapsed = last.time - first.time;
    const velocity = { x: (last.x - first.x) / elapsed, y: (last.y - first.y) / elapsed };
    const measured = speed(velocity);
    if (measured < MIN_SPEED) return null;
    const scale = Math.min(1, MAX_SPEED / measured);
    return { x: velocity.x * scale, y: velocity.y * scale };
  }
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

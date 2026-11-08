import { previewSource } from "./preview";

/** Quiet time that counts as a pause in the stream. The store folds deltas
 *  every 48 ms, so a reply still arriving changes the source more often. */
export const PAUSE_MS = 200;
/** A stream that never pauses still previews this often. */
export const MAX_WAIT_MS = 1000;

export interface Clock {
  now: () => number;
  set: (fn: () => void, ms: number) => unknown;
  clear: (timer: unknown) => void;
}

const REAL: Clock = {
  now: () => Date.now(),
  set: (fn, ms) => setTimeout(fn, ms),
  clear: (timer) => clearTimeout(timer as ReturnType<typeof setTimeout>),
};

/**
 * When a streaming fence is transpiled for a preview: at a pause in the
 * stream, or `MAX_WAIT_MS` after the last try when it never pauses; one
 * transpile at a time (each is an IPC round trip of about 24 ms), and never
 * the same repaired prefix twice. `try_` runs one candidate; the scheduler
 * looks again once it settles, in case the source moved on meanwhile.
 */
export class PreviewScheduler {
  private tried = "";
  private running = false;
  private timer: unknown = null;
  private lastTry = Number.NEGATIVE_INFINITY;
  private disposed = false;

  constructor(
    private readonly read: () => string,
    private readonly try_: (candidate: string) => Promise<void>,
    private readonly clock: Clock = REAL,
  ) {}

  /** The source changed. */
  poke() {
    if (this.disposed) return;
    const now = this.clock.now();
    const due = Math.min(now + PAUSE_MS, Math.max(now, this.lastTry + MAX_WAIT_MS));
    if (this.timer != null) this.clock.clear(this.timer);
    this.timer = this.clock.set(() => this.fire(), due - now);
  }

  dispose() {
    this.disposed = true;
    if (this.timer != null) this.clock.clear(this.timer);
    this.timer = null;
  }

  private fire() {
    this.timer = null;
    if (this.running || this.disposed) return;
    const candidate = previewSource(this.read());
    if (candidate == null || candidate === this.tried) return;
    this.tried = candidate;
    this.running = true;
    this.lastTry = this.clock.now();
    void this.try_(candidate).finally(() => {
      this.running = false;
      if (!this.disposed && previewSource(this.read()) !== this.tried) this.poke();
    });
  }
}

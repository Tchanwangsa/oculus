/**
 * The queues and caches behind the card pictures that are not a PDF page or a
 * file on disk: drawings rendered from their file and text previews. A queue
 * runs its jobs newest first and skips a job whose cards have all left the
 * screen (`pdfRender.ts` does the same for PDF pages). A finished picture is
 * kept in memory under the file's path, size and modified time, so scrolling
 * back or re-entering a folder never draws it again; a changed file gets a
 * new key. Concurrent asks for one key share one load.
 */

/** Device pixels across a card's picture. */
export const PICTURE_WIDTH = 360;

export interface FileStamp {
  path: string;
  size: number;
  modified: number | null;
}

export const stampKey = ({ path, size, modified }: FileStamp) => `${path}\u0000${size}\u0000${modified ?? ""}`;

/** Thrown to a load whose cards all left before its turn; never cached. */
export class Skipped extends Error {}

interface Job {
  run: () => Promise<void>;
  live: () => boolean;
  skip: () => void;
}

export type JobQueue = <T>(run: () => Promise<T>, live: () => boolean) => Promise<T>;

export function createJobQueue(slots: number): JobQueue {
  const queue: Job[] = [];
  let running = 0;
  const pump = () => {
    while (running < slots && queue.length) {
      const job = queue.pop()!;
      if (!job.live()) { job.skip(); continue; }
      running += 1;
      // A macrotask between jobs, so a run of main-thread renders never holds input back.
      job.run().finally(() => setTimeout(() => { running -= 1; pump(); }));
    }
  };
  return (run, live) => new Promise((resolve, reject) => {
    queue.push({ live, run: () => run().then(resolve, reject), skip: () => reject(new Skipped()) });
    pump();
  });
}

interface Pending<T> { promise?: Promise<T>; askers: Set<() => boolean> }

/** Finished pictures, least recently used dropped first; `evict` runs on each value dropped. */
export class PictureCache<T> {
  private entries = new Map<string, T>();
  private pending = new Map<string, Pending<T>>();
  constructor(private max: number, private evict: (value: T) => void = () => {}) {}

  /**
   * The value for `key`, or the one `make` makes (and keeps; a rejection is not
   * kept). `make` gets a `live` that is true while any asker's is, for its queue;
   * once it turns false the next ask starts a fresh load.
   */
  load(key: string, make: (live: () => boolean) => Promise<T>, live: () => boolean): Promise<T> {
    if (this.entries.has(key)) return Promise.resolve(this.touch(key));
    const waiting = this.pending.get(key);
    if (waiting?.promise) {
      waiting.askers.add(live);
      return waiting.promise;
    }
    const entry: Pending<T> = { askers: new Set([live]) };
    const done = () => { if (this.pending.get(key) === entry) this.pending.delete(key); };
    const anyLive = () => {
      const on = [...entry.askers].some((ask) => ask());
      if (!on) done();
      return on;
    };
    this.pending.set(key, entry);
    entry.promise = make(anyLive)
      .then((value) => { this.set(key, value); return value; })
      .finally(done);
    return entry.promise;
  }

  /** The kept value for `key`, if any, now: a card that mounts on a picture
   *  already made draws it in its first frame instead of its glyph. */
  peek(key: string): T | undefined {
    return this.entries.has(key) ? this.touch(key) : undefined;
  }

  private touch(key: string): T {
    const value = this.entries.get(key)!;
    this.entries.delete(key);
    this.entries.set(key, value);
    return value;
  }

  set(key: string, value: T) {
    const old = this.entries.get(key);
    this.entries.delete(key);
    if (old !== undefined && old !== value) this.evict(old);
    this.entries.set(key, value);
    while (this.entries.size > this.max) {
      const [oldest, dropped] = this.entries.entries().next().value!;
      this.entries.delete(oldest);
      this.evict(dropped);
    }
  }
}

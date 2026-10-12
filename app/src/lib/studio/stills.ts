// Object URLs for the stills `studio_frame` and `studio_thumbnail` return,
// shared by every tile that shows the same one. Each is a render in Rust, so
// a few run at a time and an answer is kept until the cache is full.

const MAX_CACHED = 300;
const MAX_RUNNING = 4;

const cache = new Map<string, Promise<string>>();
const waiting: Array<() => void> = [];
let running = 0;

function slot(): Promise<void> {
  if (running < MAX_RUNNING) {
    running++;
    return Promise.resolve();
  }
  return new Promise((go) => waiting.push(() => go()));
}

function release() {
  const next = waiting.shift();
  if (next) next();
  else running--;
}

/** The still's `blob:` URL; `key` must change whenever the picture would, so
 *  a re-render shows its new frame. Rejects with `load`'s sentence, and a
 *  rejection is not cached, so the next ask tries again. */
export function stillUrl(key: string, load: () => Promise<Blob>): Promise<string> {
  const hit = cache.get(key);
  if (hit) return hit;
  const made = (async () => {
    await slot();
    try {
      return URL.createObjectURL(await load());
    } finally {
      release();
    }
  })();
  cache.set(key, made);
  made.catch(() => cache.delete(key));
  if (cache.size > MAX_CACHED) {
    const [oldest] = cache.keys();
    const gone = cache.get(oldest);
    cache.delete(oldest);
    void gone?.then((url) => URL.revokeObjectURL(url)).catch(() => {});
  }
  return made;
}

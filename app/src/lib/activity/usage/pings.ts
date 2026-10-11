/** How often `useActivityPing` reports each kind. */
export const PING_INTERVAL_MS = 30_000;

/** A leading-edge gate: true at most once per `intervalMs`. `now` is
 *  injectable for tests. */
export function createThrottle(intervalMs: number, now: () => number = Date.now): () => boolean {
  let last = -Infinity;
  return () => {
    const t = now();
    if (t - last < intervalMs) return false;
    last = t;
    return true;
  };
}

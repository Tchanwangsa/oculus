/** Waits this long after the last keystroke before searching. */
export const SEARCH_DEBOUNCE_MS = 150;

/**
 * The thread column's search, debounced: `set` each keystroke; `onResults`
 * gets the newest query's results, or null when the box is cleared (at once,
 * so the view comes back without waiting). A slow answer to an older query
 * never overwrites a newer one.
 */
export function debouncedSearch<T>(
  search: (q: string) => Promise<T>,
  onResults: (results: T | null, q: string) => void,
  delay = SEARCH_DEBOUNCE_MS,
) {
  let timer: ReturnType<typeof setTimeout> | null = null;
  let latest = 0;
  const cancel = () => {
    if (timer != null) clearTimeout(timer);
    timer = null;
  };
  return {
    set(q: string) {
      cancel();
      const asked = ++latest;
      if (!q.trim()) {
        onResults(null, q);
        return;
      }
      timer = setTimeout(() => {
        timer = null;
        search(q)
          .then((results) => asked === latest && onResults(results, q))
          .catch((e) => console.error("thread search failed", e));
      }, delay);
    },
    /** Drops the pending search and any answer still on its way. */
    dispose() {
      cancel();
      latest++;
    },
  };
}

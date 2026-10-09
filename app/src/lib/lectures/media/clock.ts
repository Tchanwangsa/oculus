/** `m:ss`, or `h:mm:ss` past the hour or with `forceHours`. */
export function fmtClockSecs(secs: number, forceHours = false): string {
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const s = Math.floor(secs % 60);
  if (h > 0 || forceHours) {
    return `${h}:${m.toString().padStart(2, "0")}:${s.toString().padStart(2, "0")}`;
  }
  return `${m}:${s.toString().padStart(2, "0")}`;
}

/** Index of the span `t` falls in over ordered `starts` (cues or chapters),
 *  or -1 before the first. */
export function spanAt(starts: number[], t: number): number {
  if (Number.isNaN(t)) return -1;
  // Starts are chronological; the upper bound also selects the last duplicate.
  let low = 0;
  let high = starts.length;
  while (low < high) {
    const middle = low + Math.floor((high - low) / 2);
    if (starts[middle] <= t) low = middle + 1;
    else high = middle;
  }
  return low - 1;
}

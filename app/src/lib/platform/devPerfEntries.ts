/**
 * React's dev build records a `performance.measure` entry, with a props diff
 * as its detail, for every component render. WebKit keeps every entry, so a
 * few hours of `tauri dev` holds millions of them (gigabytes). Nothing reads
 * them, so dev clears the buffer every few seconds; production React records none.
 */
if (import.meta.env.DEV) {
  setInterval(() => performance.clearMeasures(), 5_000);
}

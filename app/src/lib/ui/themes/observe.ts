// Reads of the applied theme for code that samples colours (a canvas, a
// library's SVG): nothing re-renders it when the tokens change, so it
// subscribes here.

/** Whether `.dark` is on. */
export function isDark(): boolean {
  return document.documentElement.classList.contains("dark");
}

function observe(attributes: string[], onChange: () => void): () => void {
  const observer = new MutationObserver(onChange);
  observer.observe(document.documentElement, { attributes: true, attributeFilter: attributes });
  return () => observer.disconnect();
}

/** Calls `onChange` when `isDark` may change; for `useSyncExternalStore`. */
export function subscribeDark(onChange: () => void): () => void {
  return observe(["class"], onChange);
}

/** The applied mode and theme as one string, e.g. `D:nord`: it changes
 *  whenever the token values do, apart from the accent and code overrides. */
export function themeKey(): string {
  const root = document.documentElement;
  return `${isDark() ? "D" : "L"}:${root.dataset.theme ?? ""}`;
}

/** Calls `onChange` when `themeKey` may change. */
export function subscribeTheme(onChange: () => void): () => void {
  return observe(["class", "data-theme"], onChange);
}

export type Theme = "light" | "dark" | "system";

const STORAGE_KEY = "oculus-theme";

export function getStoredTheme(): Theme {
  return (localStorage.getItem(STORAGE_KEY) as Theme) ?? "light";
}

export function applyTheme(theme: Theme) {
  const root = document.documentElement;
  const dark =
    theme === "dark" ||
    (theme === "system" && window.matchMedia("(prefers-color-scheme: dark)").matches);

  root.classList.toggle("dark", dark);
  localStorage.setItem(STORAGE_KEY, theme);
}

/** Whether `.dark` is on. Code that samples colours (a canvas, a library's
 *  SVG) also needs `subscribeDark`, since nothing re-renders on a flip. */
export function isDark(): boolean {
  return document.documentElement.classList.contains("dark");
}

/** Calls `onChange` when `isDark` would change; for `useSyncExternalStore`. */
export function subscribeDark(onChange: () => void): () => void {
  const observer = new MutationObserver(onChange);
  observer.observe(document.documentElement, { attributes: true, attributeFilter: ["class"] });
  return () => observer.disconnect();
}

/** Re-applies "system" when the OS preference flips; `applyTheme` resolves it
 *  only once. */
export function watchSystemTheme(): () => void {
  const query = window.matchMedia("(prefers-color-scheme: dark)");
  const onChange = () => {
    if (getStoredTheme() === "system") applyTheme("system");
  };
  query.addEventListener("change", onChange);
  return () => query.removeEventListener("change", onChange);
}

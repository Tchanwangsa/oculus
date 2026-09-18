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

/** Whether the dark palette is the one currently in the cascade.
 *
 *  Anything that has to *read* a colour rather than name it in a class — an
 *  SVG a library draws for us, a canvas — needs both this and
 *  [`subscribeDark`], because the tokens it samples change under it when the
 *  class on `<html>` flips and nothing re-renders on its own. */
export function isDark(): boolean {
  return document.documentElement.classList.contains("dark");
}

/** Call `onChange` whenever [`isDark`] would answer differently. Shaped for
 *  `useSyncExternalStore`, which is the only caller so far. */
export function subscribeDark(onChange: () => void): () => void {
  const observer = new MutationObserver(onChange);
  observer.observe(document.documentElement, { attributes: true, attributeFilter: ["class"] });
  return () => observer.disconnect();
}

/**
 * Keep "system" honest while the app is open.
 *
 * `applyTheme` resolves the OS preference once, at the moment it is called, so
 * without this a machine that flips to dark at sunset stays light until the
 * next launch. Only "system" follows the OS — an explicit choice is a choice.
 */
export function watchSystemTheme(): () => void {
  const query = window.matchMedia("(prefers-color-scheme: dark)");
  const onChange = () => {
    if (getStoredTheme() === "system") applyTheme("system");
  };
  query.addEventListener("change", onChange);
  return () => query.removeEventListener("change", onChange);
}

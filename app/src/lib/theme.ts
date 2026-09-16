export type Theme = "light" | "dark" | "system";

const STORAGE_KEY = "oculus-theme";

export function getStoredTheme(): Theme {
  return (localStorage.getItem(STORAGE_KEY) as Theme) ?? "light";
}

export function applyTheme(theme: Theme) {
  const root = document.documentElement;
  const isDark =
    theme === "dark" ||
    (theme === "system" && window.matchMedia("(prefers-color-scheme: dark)").matches);

  root.classList.toggle("dark", isDark);
  localStorage.setItem(STORAGE_KEY, theme);
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

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

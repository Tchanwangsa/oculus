import { useSyncExternalStore } from "react";
import { applyAppearance } from "./apply";
import {
  APPEARANCE_KEY,
  LEGACY_THEME_KEY,
  readAppearance,
  validateAppearance,
  type AppearancePrefs,
} from "./prefs";

let current: AppearancePrefs | null = null;
const listeners = new Set<() => void>();

function load(): AppearancePrefs {
  let raw: string | null = null;
  let legacy: string | null = null;
  try {
    raw = localStorage.getItem(APPEARANCE_KEY);
    legacy = localStorage.getItem(LEGACY_THEME_KEY);
  } catch {
    // Blocked storage reads as nothing stored.
  }
  return readAppearance(raw, legacy);
}

/** The stored prefs, read once and then kept; the same object until a set. */
export function getAppearance(): AppearancePrefs {
  return (current ??= load());
}

/** Merges `patch`, validates, persists, applies and notifies subscribers. */
export function setAppearance(patch: Partial<AppearancePrefs>) {
  current = validateAppearance({ ...getAppearance(), ...patch });
  try {
    localStorage.setItem(APPEARANCE_KEY, JSON.stringify(current));
  } catch {
    // The live window still takes the change without storage.
  }
  applyAppearance(current);
  for (const listener of listeners) listener();
}

export function subscribeAppearance(onChange: () => void): () => void {
  listeners.add(onChange);
  return () => listeners.delete(onChange);
}

export function useAppearance(): AppearancePrefs {
  return useSyncExternalStore(subscribeAppearance, getAppearance);
}

/** Re-applies the prefs when the OS flips light/dark, which "system" mode
 *  follows; `applyAppearance` resolves it only once per call. */
export function watchSystemTheme(): () => void {
  const query = window.matchMedia("(prefers-color-scheme: dark)");
  const onChange = () => {
    const prefs = getAppearance();
    if (prefs.mode === "system") applyAppearance(prefs);
  };
  query.addEventListener("change", onChange);
  return () => query.removeEventListener("change", onChange);
}

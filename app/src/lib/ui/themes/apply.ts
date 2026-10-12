import { invoke } from "@tauri-apps/api/core";
import { isMac } from "@/lib/platform";
import { accentTokens } from "./accent";
import { syntaxTokens, themeTokens } from "./derive";
import { withAlpha } from "./oklab";
import type { AppearancePrefs, Frame } from "./prefs";
import { getTheme } from "./registry";
import type { ThemeMode, TokenMap } from "./types";

/** How much of the ground each frame keeps; the rest is the native glass
 *  behind the webview. The card, popovers and dialogs stay opaque. */
export const FRAME_ALPHA: Record<Frame, number> = { solid: 1, frosted: 0.55, clear: 0.2 };

/** `--font-mono` from `styles/theme.css`, which a chosen code font leads. */
export const MONO_STACK = 'ui-monospace, "SF Mono", SFMono-Regular, Menlo, "Cascadia Mono", Consolas, monospace';

export function prefersDark(): boolean {
  return typeof window !== "undefined" && !!window.matchMedia?.("(prefers-color-scheme: dark)").matches;
}

export function resolveMode(prefs: AppearancePrefs, systemDark = prefersDark()): ThemeMode {
  if (prefs.mode === "system") return systemDark ? "dark" : "light";
  return prefs.mode;
}

/** Every inline custom property `prefs` sets in `mode`. The built-in theme
 *  with no overrides and a solid frame sets none: the stylesheet is it. */
export function appearanceTokens(prefs: AppearancePrefs, mode: ThemeMode): TokenMap {
  const theme = getTheme(prefs[mode], mode);
  const tokens = { ...themeTokens(theme) };
  if (prefs.accent) Object.assign(tokens, accentTokens(prefs.accent, theme.palette, mode));
  const code = mode === "light" ? prefs.codeLight : prefs.codeDark;
  if (code !== "match") Object.assign(tokens, syntaxTokens(getTheme(code, mode).syntax));
  if (prefs.codeFont) tokens["--font-mono"] = `"${prefs.codeFont}", ${MONO_STACK}`;
  const alpha = FRAME_ALPHA[prefs.frame];
  if (alpha < 1) tokens["--color-frame"] = withAlpha(theme.palette.ground, alpha);
  return tokens;
}

let written = new Set<string>();
let applied = 0;
/** The last `native_set_glass` arguments sent; a drag on the accent re-applies
 *  every step and the native side needn't hear it. */
let sent = "";

/** Toggles `.dark`, writes the theme's tokens inline on `<html>` (clearing
 *  what the last call wrote) and asks the native side for glass. The only
 *  writer of the `.dark` class and of `data-theme`. */
export function applyAppearance(prefs: AppearancePrefs) {
  const root = document.documentElement;
  const mode = resolveMode(prefs);
  const tokens = appearanceTokens(prefs, mode);
  root.classList.toggle("dark", mode === "dark");
  for (const name of written) if (!(name in tokens)) root.style.removeProperty(name);
  for (const [name, value] of Object.entries(tokens)) root.style.setProperty(name, value);
  written = new Set(Object.keys(tokens));
  root.dataset.theme = getTheme(prefs[mode], mode).id;

  if (!isMac) return;
  const glass = prefs.frame !== "solid";
  const key = `${glass}:${mode}`;
  if (key === sent) return;
  sent = key;
  const run = ++applied;
  invoke("native_set_glass", { enabled: glass, dark: mode === "dark" }).catch(() => {
    sent = "";
    // No glass behind the webview: a see-through ground would show the
    // desktop, so the frame goes back to the stylesheet's opaque ground.
    if (run !== applied || !glass) return;
    root.style.removeProperty("--color-frame");
    written.delete("--color-frame");
  });
}

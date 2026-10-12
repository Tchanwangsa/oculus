import type { ITheme } from "@xterm/xterm";

import { isDark } from "@/lib/ui/themes/observe";
import { MONO_STACK } from "@/lib/ui/themes/apply";
import { withAlpha } from "@/lib/ui/themes/oklab";

/**
 * xterm draws on a canvas and takes resolved colours, so the theme's tokens
 * are read off `<html>` (every one a hex, docs/ui.md) and read again when
 * the theme or the code font changes. The ANSI colours borrow the semantic
 * and reference tokens, so a terminal follows the app's theme.
 */
function token(name: string, fallback: string): string {
  const v = getComputedStyle(document.documentElement).getPropertyValue(`--${name}`).trim();
  return v || fallback;
}

export function xtermTheme(): ITheme {
  const dark = isDark();
  const fg = token("color-foreground", dark ? "#ececec" : "#0d0d0d");
  const muted = token("color-muted-foreground", "#757575");
  const raised = token("color-surface-overlay", dark ? "#3a3a3a" : "#e0e0e0");
  const brand = token("color-brand", "#5e6ad2");
  const red = token("color-destructive", "#dc2626");
  const green = token("color-success", "#16a34a");
  const yellow = token("color-warning", "#b45309");
  const blue = token("color-reference-document", "#3682b6");
  const magenta = token("color-reference-page", "#8654c8");
  const cyan = token("color-reference-lesson", "#388a82");
  return {
    background: token("color-card", dark ? "#1c1c1c" : "#ffffff"),
    foreground: fg,
    cursor: brand,
    cursorAccent: token("color-card", "#ffffff"),
    selectionBackground: brand.startsWith("#") ? withAlpha(brand, 0.3) : brand,
    black: dark ? raised : fg,
    red,
    green,
    yellow,
    blue,
    magenta,
    cyan,
    white: dark ? fg : muted,
    brightBlack: muted,
    brightRed: red,
    brightGreen: green,
    brightYellow: yellow,
    brightBlue: blue,
    brightMagenta: magenta,
    brightCyan: cyan,
    brightWhite: dark ? fg : raised,
  };
}

/** The code font, as `CodeEditor` uses it. */
export const xtermFont = () => token("font-mono", MONO_STACK);

import { parseHex, toHex } from "@/lib/ui/color";
import { isMac } from "@/lib/platform";
import { BUILTIN, hasTheme } from "./registry";

export type AppearanceMode = "light" | "dark" | "system";
/** The window ground around the card: opaque, or see-through onto the
 *  native glass behind the webview (macOS only). */
export type Frame = "solid" | "frosted" | "clear";

export interface AppearancePrefs {
  mode: AppearanceMode;
  /** Theme ids for each resolved mode. */
  light: string;
  dark: string;
  /** `#rrggbb` that replaces the theme's accent, or null for the theme's own. */
  accent: string | null;
  frame: Frame;
  /** A font family put ahead of the monospace stack; "" for the stack alone. */
  codeFont: string;
}

export const APPEARANCE_KEY = "oculus-appearance";
/** The pre-themes key, holding only "light" | "dark" | "system". */
export const LEGACY_THEME_KEY = "oculus-theme";

const MODES: readonly AppearanceMode[] = ["light", "dark", "system"];
const FRAMES: readonly Frame[] = ["solid", "frosted", "clear"];
/** Characters that could close the CSS string `--font-mono` wraps it in. */
const UNSAFE_FONT = /["'\\;{}<>]/;

export function defaultAppearance(mac = isMac): AppearancePrefs {
  return {
    mode: "light",
    light: BUILTIN.light.id,
    dark: BUILTIN.dark.id,
    accent: null,
    frame: mac ? "frosted" : "solid",
    codeFont: "",
  };
}

const isMode = (v: unknown): v is AppearanceMode => MODES.includes(v as AppearanceMode);

/** `#rrggbb` (lowercase) for an opaque hex colour, else null. */
export function normalizeAccent(v: unknown): string | null {
  const c = typeof v === "string" ? parseHex(v) : null;
  return c && c.a === 1 ? toHex(c) : null;
}

/** Fills and corrects whatever `raw` holds: an unknown theme id falls back to
 *  the built-in, a bad accent to null, and a frame other than solid only
 *  survives on macOS. */
export function validateAppearance(raw: unknown, mac = isMac): AppearancePrefs {
  const d = defaultAppearance(mac);
  const v = (raw && typeof raw === "object" ? raw : {}) as Record<string, unknown>;
  const font = typeof v.codeFont === "string" ? v.codeFont.trim() : "";
  const frame = FRAMES.includes(v.frame as Frame) ? (v.frame as Frame) : d.frame;
  return {
    mode: isMode(v.mode) ? v.mode : d.mode,
    light: hasTheme(v.light, "light") ? v.light : d.light,
    dark: hasTheme(v.dark, "dark") ? v.dark : d.dark,
    accent: normalizeAccent(v.accent),
    frame: mac ? frame : "solid",
    codeFont: UNSAFE_FONT.test(font) ? "" : font,
  };
}

/** Prefs from the stored JSON, or, before any is stored, the legacy theme
 *  key's mode on top of the defaults. */
export function readAppearance(raw: string | null, legacy: string | null, mac = isMac): AppearancePrefs {
  if (raw != null) {
    try {
      return validateAppearance(JSON.parse(raw), mac);
    } catch {
      // Unreadable JSON reads as nothing stored.
    }
  }
  return validateAppearance({ mode: legacy }, mac);
}

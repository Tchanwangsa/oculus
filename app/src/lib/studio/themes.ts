import { invoke } from "@tauri-apps/api/core";

import type { ComponentLevel, Snapshot } from "@/lib/studio/components";

// The themes a video can use (`app/src-tauri/src/studio/commands/themes/`):
// the bundled ones and the global and the subject's libraries, one row per
// name from the nearest level; a theme's text, validated writes with a
// history, removal, and each library's default theme (`library.json`). A
// name is lowercase letters, digits and `-`.

/** Where a theme is defined; a nearer level replaces a farther one of the
 *  same name. */
export type ThemeLevel = "bundled" | "global" | "subject";

export interface ThemeInfo {
  name: string;
  level: ThemeLevel;
  /** Up to four CSS colours (background, text, accent, surface) the style
   *  chip paints; empty when the theme does not parse. */
  swatch: string[];
  /** The theme file as components see it (`ctx.theme`); null when it does
   *  not parse. */
  tokens: Record<string, unknown> | null;
  /** Why the theme does not parse; the page shows it on the row. */
  problem: string | null;
  /** The farther level this library file replaces under the same name: a
   *  subject's "own version" of a global theme. */
  shadows: ThemeLevel | null;
}

export interface ThemeText {
  level: ThemeLevel;
  text: string;
}

export interface ThemeDefaults {
  /** The global library's `defaultTheme`, as written. */
  global: string | null;
  /** The subject library's, when a subject was asked about. */
  subject: string | null;
  /** The theme a new video there takes. */
  effective: string;
}

/** The themes in effect for `subject` (a course code), or the global set;
 *  `default` first, then by name. */
export function listThemes(subject?: string | null): Promise<ThemeInfo[]> {
  return invoke<ThemeInfo[]>("studio_themes", { subject: subject ?? null });
}

/** The theme's file text from the level that wins (a bundled theme's text as
 *  compiled in). */
export function themeSource(name: string, subject?: string | null): Promise<ThemeText> {
  return invoke<ThemeText>("studio_theme_source", { name, subject: subject ?? null });
}

/** Writes `text` as the theme at `level`. Rejects, writing nothing, when it
 *  does not parse or names a bundled theme; the text it replaces goes to the
 *  history. */
export function writeTheme(
  name: string,
  level: ComponentLevel,
  subject: string | null | undefined,
  text: string,
): Promise<ThemeText> {
  return invoke<ThemeText>("studio_theme_write", { name, level, subject: subject ?? null, text });
}

/** Removes the library theme at `level`; its text goes to the history first. */
export function deleteTheme(name: string, level: ComponentLevel, subject?: string | null): Promise<void> {
  return invoke<void>("studio_theme_delete", { name, level, subject: subject ?? null });
}

/** The theme's earlier texts at `level`, newest first. */
export function themeHistory(name: string, level: ComponentLevel, subject?: string | null): Promise<Snapshot[]> {
  return invoke<Snapshot[]>("studio_theme_history", { name, level, subject: subject ?? null });
}

/** Writes snapshot `id` back; the text it replaces becomes the newest one. */
export function restoreTheme(
  name: string,
  level: ComponentLevel,
  subject: string | null | undefined,
  id: string,
): Promise<ThemeText> {
  return invoke<ThemeText>("studio_theme_restore", { name, level, subject: subject ?? null, id });
}

/** The libraries' default themes for `subject` (or the global library). */
export function themeDefaults(subject?: string | null): Promise<ThemeDefaults> {
  return invoke<ThemeDefaults>("studio_theme_defaults", { subject: subject ?? null });
}

/** Makes `name` the default of the library at `level`, or clears it. */
export function setDefaultTheme(
  level: ComponentLevel,
  subject: string | null | undefined,
  name: string | null,
): Promise<ThemeDefaults> {
  return invoke<ThemeDefaults>("studio_theme_set_default", { level, subject: subject ?? null, name });
}

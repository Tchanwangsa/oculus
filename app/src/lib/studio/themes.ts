import { invoke } from "@tauri-apps/api/core";

// The themes a video can use (`app/src-tauri/src/studio/commands/themes/`):
// the bundled ones and the global and the subject's libraries, one row per
// name from the nearest level.

export interface ThemeInfo {
  name: string;
  /** Where the theme is defined; a nearer level replaces a farther one of
   *  the same name. */
  level: "bundled" | "global" | "subject";
  /** Up to four CSS colours (background, text, accent, surface) the style
   *  chip paints; empty when the theme does not parse. */
  swatch: string[];
  /** The theme file as components see it (`ctx.theme`); null when it does
   *  not parse. */
  tokens: Record<string, unknown> | null;
  /** Why the theme does not parse; the page shows it on the row. */
  problem: string | null;
}

/** The themes in effect for `subject` (a course code), or the global set;
 *  `default` first, then by name. */
export function listThemes(subject?: string | null): Promise<ThemeInfo[]> {
  return invoke<ThemeInfo[]>("studio_themes", { subject: subject ?? null });
}

import { invoke } from "@tauri-apps/api/core";

// The themes a video can use (`app/src-tauri/src/studio/commands/`): the
// bundled ones, then the global and the subject's libraries, nearest last.

export interface ThemeInfo {
  name: string;
  /** Where the theme is defined; a nearer level replaces a farther one of
   *  the same name. */
  level: "bundled" | "global" | "subject";
  /** Three or four CSS colours (background first) the style chip paints. */
  swatch: string[];
}

/** The themes in effect for `subject` (a course code), or the global set. */
export function listThemes(subject?: string | null): Promise<ThemeInfo[]> {
  return invoke<ThemeInfo[]>("studio_themes", { subject: subject ?? null });
}

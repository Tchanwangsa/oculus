import { OCULUS_DARK, OCULUS_LIGHT } from "./builtin";
import { DARK_SAMPLES } from "./samplesDark";
import { LIGHT_SAMPLES } from "./samplesLight";
import type { ThemeMode, ThemeSpec } from "./types";

/** Every theme, built-ins first in each mode. */
export const THEMES: readonly ThemeSpec[] = [OCULUS_LIGHT, ...LIGHT_SAMPLES, OCULUS_DARK, ...DARK_SAMPLES];

const BY_ID = new Map(THEMES.map((t) => [t.id, t]));

export const BUILTIN: Record<ThemeMode, ThemeSpec> = { light: OCULUS_LIGHT, dark: OCULUS_DARK };

export function themesFor(mode: ThemeMode): ThemeSpec[] {
  return THEMES.filter((t) => t.mode === mode);
}

/** The theme `id` names when it exists in `mode`, else that mode's built-in. */
export function getTheme(id: string | null | undefined, mode: ThemeMode): ThemeSpec {
  const theme = id ? BY_ID.get(id) : undefined;
  return theme?.mode === mode ? theme : BUILTIN[mode];
}

/** Whether `id` names a theme in `mode`. */
export function hasTheme(id: unknown, mode: ThemeMode): id is string {
  return typeof id === "string" && BY_ID.get(id)?.mode === mode;
}

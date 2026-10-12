import type { CSSProperties } from "react";
import { accentTokens, derive, type ThemeSpec } from "@/lib/ui/themes";

/**
 * A theme's tokens as inline custom properties, so token utilities inside a
 * preview draw that theme whatever mode the page is in. `accent` is the
 * user's override.
 */
export function previewStyle(theme: ThemeSpec, accent: string | null): CSSProperties {
  return {
    ...derive(theme),
    ...(accent ? accentTokens(accent, theme.palette, theme.mode) : {}),
  } as CSSProperties;
}

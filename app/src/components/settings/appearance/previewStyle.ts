import type { CSSProperties } from "react";
import { accentTokens, derive, syntaxTokens, type SyntaxColors, type ThemeSpec } from "@/lib/ui/themes";

/**
 * A theme's tokens as inline custom properties, so token utilities inside a
 * preview draw that theme whatever mode the page is in. `accent` is the
 * user's override; `syntax` swaps in a code theme's colours.
 */
export function previewStyle(theme: ThemeSpec, accent: string | null, syntax?: SyntaxColors): CSSProperties {
  return {
    ...derive(theme),
    ...(accent ? accentTokens(accent, theme.palette, theme.mode) : {}),
    ...(syntax ? syntaxTokens(syntax) : {}),
  } as CSSProperties;
}

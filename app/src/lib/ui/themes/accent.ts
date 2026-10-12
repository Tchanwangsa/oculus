import { contrast, fromLab, luminance, shiftLightness, toLab } from "./oklab";
import type { ThemeMode, ThemePalette, TokenMap } from "./types";

/** OKLab lightness steps for a fill's hover and press, fitted to the indigo in
 *  `colors.css` / `dark.css`: light darkens both; dark lightens on hover. */
const STATES: Record<ThemeMode, { hover: number; active: number }> = {
  light: { hover: -0.053, active: -0.121 },
  dark: { hover: 0.073, active: -0.082 },
};

/** How far `brand-muted` (a chip's tint) sits from the card towards the
 *  accent, in lightness and in chroma, fitted the same way. */
const TINT: Record<ThemeMode, { L: number; C: number }> = {
  light: { L: 0.1, C: 0.12 },
  dark: { L: 0.05, C: 0.38 },
};

/** Text on a solid `fill`: white or the theme's darkest colour, whichever
 *  reads better, and pure black when neither reaches 4.5:1. */
export function textOn(fill: string, palette: ThemePalette): string {
  const darkest = [palette.ground, palette.card, palette.foreground].reduce((p, q) =>
    luminance(q) < luminance(p) ? q : p,
  );
  const ranked = ["#ffffff", darkest, "#000000"]
    .map((c) => ({ c, ratio: contrast(fill, c) }))
    .sort((p, q) => q.ratio - p.ratio);
  const pair = ranked.find((r) => r.c !== "#000000" && r.ratio >= 4.5) ?? ranked[0];
  return pair.c;
}

/** The fill (`primary`), the accent (`brand`) and the focus ring, all from
 *  one colour. Used for a theme's own accent and for the accent override. */
export function accentTokens(accent: string, palette: ThemePalette, mode: ThemeMode): TokenMap {
  const { hover, active } = STATES[mode];
  const onAccent = textOn(accent, palette);
  const card = toLab(palette.card);
  const tone = toLab(accent);
  const tint = TINT[mode];
  const muted = fromLab({
    L: card.L + (tone.L - card.L) * tint.L,
    a: card.a + (tone.a - card.a) * tint.C,
    b: card.b + (tone.b - card.b) * tint.C,
  });
  const hoverFill = shiftLightness(accent, hover);
  return {
    "--color-ring": accent,
    "--color-primary": accent,
    "--color-primary-hover": hoverFill,
    "--color-primary-active": shiftLightness(accent, active),
    "--color-primary-foreground": onAccent,
    "--color-brand": accent,
    "--color-brand-hover": hoverFill,
    "--color-brand-muted": muted,
    "--color-brand-foreground": onAccent,
  };
}

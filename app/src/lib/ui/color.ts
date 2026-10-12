// Colours as the colour control edits them: hex text (`#rgb`, `#rrggbb` or
// `#rrggbbaa`, the forms Studio's themes accept) and HSV, the picker's pad
// (saturation across, value up) and hue strip. Channels are 0-255, alpha
// 0-1, hue 0-360, saturation and value 0-1.

export interface Rgba {
  r: number;
  g: number;
  b: number;
  a: number;
}

export interface Hsv {
  h: number;
  s: number;
  v: number;
}

const HEX = /^#?([0-9a-f]{3}|[0-9a-f]{6}|[0-9a-f]{8})$/i;

/** `#rgb`, `#rrggbb` or `#rrggbbaa` (the `#` optional), else null. */
export function parseHex(text: string): Rgba | null {
  const m = HEX.exec(text.trim());
  if (!m) return null;
  let digits = m[1];
  if (digits.length === 3) digits = [...digits].map((d) => d + d).join("");
  const at = (i: number) => parseInt(digits.slice(i, i + 2), 16);
  return { r: at(0), g: at(2), b: at(4), a: digits.length === 8 ? at(6) / 255 : 1 };
}

const pair = (n: number) => Math.round(Math.min(255, Math.max(0, n))).toString(16).padStart(2, "0");

/** Lowercase `#rrggbb`, with an alpha pair only when the colour is not
 *  opaque. */
export function toHex({ r, g, b, a }: Rgba): string {
  const base = `#${pair(r)}${pair(g)}${pair(b)}`;
  return a >= 1 ? base : `${base}${pair(a * 255)}`;
}

export function rgbToHsv({ r, g, b }: Rgba): Hsv {
  const [R, G, B] = [r / 255, g / 255, b / 255];
  const max = Math.max(R, G, B);
  const d = max - Math.min(R, G, B);
  let h = 0;
  if (d > 0) {
    if (max === R) h = ((G - B) / d) % 6;
    else if (max === G) h = (B - R) / d + 2;
    else h = (R - G) / d + 4;
    h *= 60;
    if (h < 0) h += 360;
  }
  return { h, s: max === 0 ? 0 : d / max, v: max };
}

export function hsvToRgb({ h, s, v }: Hsv, a = 1): Rgba {
  const c = v * s;
  const hh = (((h % 360) + 360) % 360) / 60;
  const x = c * (1 - Math.abs((hh % 2) - 1));
  const [r, g, b] =
    hh < 1 ? [c, x, 0] : hh < 2 ? [x, c, 0] : hh < 3 ? [0, c, x] : hh < 4 ? [0, x, c] : hh < 5 ? [x, 0, c] : [c, 0, x];
  const m = v - c;
  return { r: Math.round((r + m) * 255), g: Math.round((g + m) * 255), b: Math.round((b + m) * 255), a };
}

/** Clamps `n` into [0, 1]. */
export const unit = (n: number) => Math.min(1, Math.max(0, n));

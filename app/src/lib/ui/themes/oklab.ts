// Colour maths for deriving theme tokens. Mixes run in OKLab, where equal
// steps of `t` read as equal steps of lightness, so a ramp built from two
// ends looks evenly spaced. Every result is a resolved `#rrggbb` string:
// mermaid reads the tokens with `getComputedStyle` and rejects `color-mix()`.

import { parseHex, toHex, type Rgba } from "@/lib/ui/color";

export interface Lab {
  L: number;
  a: number;
  b: number;
}

const toLinear = (c: number) => {
  const v = c / 255;
  return v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
};

const fromLinear = (v: number) => {
  const c = v <= 0.0031308 ? v * 12.92 : 1.055 * Math.max(0, v) ** (1 / 2.4) - 0.055;
  return Math.round(Math.min(1, Math.max(0, c)) * 255);
};

/** A hex colour as RGBA; throws on anything else, since palettes are code. */
export function rgba(hex: string): Rgba {
  const c = parseHex(hex);
  if (!c) throw new Error(`not a hex colour: ${hex}`);
  return c;
}

export function toLab(hex: string): Lab {
  const { r, g, b } = rgba(hex);
  const [R, G, B] = [toLinear(r), toLinear(g), toLinear(b)];
  const l = Math.cbrt(0.4122214708 * R + 0.5363325363 * G + 0.0514459929 * B);
  const m = Math.cbrt(0.2119034982 * R + 0.6806995451 * G + 0.1073969566 * B);
  const s = Math.cbrt(0.0883024619 * R + 0.2817188376 * G + 0.6299787005 * B);
  return {
    L: 0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s,
    a: 1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s,
    b: 0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s,
  };
}

export function fromLab({ L, a, b }: Lab): string {
  const l = (L + 0.3963377774 * a + 0.2158037573 * b) ** 3;
  const m = (L - 0.1055613458 * a - 0.0638541728 * b) ** 3;
  const s = (L - 0.0894841775 * a - 1.291485548 * b) ** 3;
  return toHex({
    r: fromLinear(4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s),
    g: fromLinear(-1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s),
    b: fromLinear(-0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s),
    a: 1,
  });
}

/** `from` moved `t` (0-1) of the way to `to`. */
export function mix(from: string, to: string, t: number): string {
  const p = toLab(from);
  const q = toLab(to);
  return fromLab({ L: p.L + (q.L - p.L) * t, a: p.a + (q.a - p.a) * t, b: p.b + (q.b - p.b) * t });
}

/** `hex` with its OKLab lightness moved by `dL`, hue and chroma kept. */
export function shiftLightness(hex: string, dL: number): string {
  const c = toLab(hex);
  return fromLab({ ...c, L: Math.min(1, Math.max(0, c.L + dL)) });
}

/** WCAG relative luminance. */
export function luminance(hex: string): number {
  const { r, g, b } = rgba(hex);
  return 0.2126 * toLinear(r) + 0.7152 * toLinear(g) + 0.0722 * toLinear(b);
}

/** WCAG contrast ratio, 1 to 21. */
export function contrast(x: string, y: string): number {
  const [hi, lo] = [luminance(x), luminance(y)].sort((p, q) => q - p);
  return (hi + 0.05) / (lo + 0.05);
}

/** `hex` at `alpha`, as `#rrggbbaa` (plain `#rrggbb` when opaque). */
export function withAlpha(hex: string, alpha: number): string {
  return toHex({ ...rgba(hex), a: alpha });
}

import { Color, SRGBColorSpace } from "three";
import { tokenProperty, type Colour } from "../colors";

const cache = new Map<string, Color>();
let probe: CanvasRenderingContext2D | null = null;

/** A CSS colour as the browser paints it, as sRGB bytes: tokens may be in a
 *  syntax `THREE.Color` can't read, and a 1×1 canvas reads them all. */
function bytes(css: string): [number, number, number] | null {
  probe ??= document.createElement("canvas").getContext("2d", { willReadFrequently: true });
  if (!probe) return null;
  probe.clearRect(0, 0, 1, 1);
  // An unreadable value leaves the last fill: start from none.
  probe.fillStyle = "transparent";
  probe.fillStyle = css;
  probe.fillRect(0, 0, 1, 1);
  const [r, g, b, a] = probe.getImageData(0, 0, 1, 1).data;
  return a ? [r, g, b] : null;
}

/**
 * `color` (or `fallback`) as a three colour, from the token the parent wrote
 * into the frame. Read once per token: a theme change rebuilds the frame.
 * A name that isn't a theme colour throws, as in `oculus/2d`. The colour is
 * shared: copy it before changing it.
 */
export function themeColour(color: Colour | undefined, fallback: Colour): Color {
  const property = tokenProperty(color, fallback);
  let colour = cache.get(property);
  if (!colour) {
    const value = getComputedStyle(document.documentElement).getPropertyValue(property).trim();
    const rgb = value ? bytes(value) : null;
    if (!rgb) throw new Error(`The theme has no ${property} to draw with`);
    colour = new Color().setRGB(rgb[0] / 255, rgb[1] / 255, rgb[2] / 255, SRGBColorSpace);
    cache.set(property, colour);
  }
  return colour;
}

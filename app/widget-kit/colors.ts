/**
 * The colours `oculus/2d` and `oculus/3d` draw in: names of the app's tokens,
 * which the parent writes into the frame for the current theme, so light and
 * dark both follow the app. Anything else (a hex, a CSS name) is an error the
 * widget shows, never a colour on screen.
 */
export type Colour =
  | "foreground"
  | "muted"
  | "brand"
  | "success"
  | "warning"
  | "danger"
  | "chart-1"
  | "chart-2"
  | "chart-3"
  | "chart-4"
  | "chart-5";

const TOKENS: Record<Colour, string> = {
  foreground: "foreground",
  muted: "muted-foreground",
  brand: "brand",
  success: "success",
  warning: "warning",
  danger: "destructive",
  "chart-1": "chart-1",
  "chart-2": "chart-2",
  "chart-3": "chart-3",
  "chart-4": "chart-4",
  "chart-5": "chart-5",
};

export const COLOURS = Object.keys(TOKENS) as Colour[];

/** The custom property `color` (or `fallback`) names: `--color-…`. */
export function tokenProperty(color: Colour | undefined, fallback: Colour): string {
  const name = color ?? fallback;
  const token = Object.prototype.hasOwnProperty.call(TOKENS, name) ? TOKENS[name] : undefined;
  if (!token) throw new Error(`"${String(name)}" isn't a theme colour; use one of ${COLOURS.join(", ")}`);
  return `--color-${token}`;
}

/** `color` (or `fallback`) as the CSS value of its token. */
export function paint(color: Colour | undefined, fallback: Colour): string {
  return `var(${tokenProperty(color, fallback)})`;
}

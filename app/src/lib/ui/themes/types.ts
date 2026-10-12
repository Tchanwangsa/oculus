export type ThemeMode = "light" | "dark";

/** The nine code-highlighting slots, `--color-syntax-<slot>`. */
export const SYNTAX_SLOTS = [
  "keyword",
  "string",
  "comment",
  "number",
  "function",
  "type",
  "property",
  "operator",
  "meta",
] as const;

export type SyntaxSlot = (typeof SYNTAX_SLOTS)[number];
export type SyntaxColors = Record<SyntaxSlot, string>;

/** The few colours a theme picks; `derive` builds every token from them.
 *  All `#rrggbb`. The ground is one step off the card, which is the reading
 *  surface; `accent` is the brand (buttons, links), not shadcn's `accent`. */
export interface ThemePalette {
  ground: string;
  card: string;
  popover: string;
  foreground: string;
  mutedForeground: string;
  border: string;
  accent: string;
  destructive?: string;
  success?: string;
  warning?: string;
}

export interface ThemeSpec {
  id: string;
  name: string;
  mode: ThemeMode;
  palette: ThemePalette;
  /** Read on the theme's `surface-raised`, where code blocks sit. */
  syntax: SyntaxColors;
  /** The stylesheet's own look (`colors.css` / `dark.css`): applying it
   *  writes no inline tokens. Its palette is for previews only. */
  builtin?: boolean;
}

/** `--color-*` custom property name to a resolved colour. */
export type TokenMap = Record<string, string>;

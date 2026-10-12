// The Themes page's draft: a theme file's JSON as plain objects, edited one
// token at a time and written back as text. The shape is studio-core's
// `theme/schema.rs` (docs/studio.md, "Fonts and themes are bundled"); the
// backend checks every write, so these helpers only keep edits well formed.

export type Tokens = Record<string, unknown>;

/** The families a theme may name: the fonts compiled into studio-core
 *  (`app/studio-core/src/fonts/mod.rs`, `BUNDLED`). */
export const THEME_FONTS = ["Inter", "Manrope", "JetBrains Mono"] as const;

/** The three `font` roles. */
export type FontRole = "heading" | "body" | "mono";

/** The values the motion tokens take (`app/studio-core/src/transition/`). */
export const TRANSITION_KINDS = ["cut", "dissolve", "push", "wipe", "zoom", "magic"] as const;
export const DIRECTIONS = ["left", "right", "up", "down"] as const;
export const MAGIC_MOVE = ["auto", "off"] as const;
export const MORPH_COLOR = ["oklch", "oklab"] as const;
export const MOTION_BLUR = ["finals", "off", "always"] as const;

const isObject = (v: unknown): v is Tokens => typeof v === "object" && v !== null && !Array.isArray(v);

/** The theme file's JSON, or the parser's sentence. */
export function parseThemeText(text: string): { tokens: Tokens } | { error: string } {
  try {
    const value: unknown = JSON.parse(text);
    return isObject(value) ? { tokens: value } : { error: "A theme file is a JSON object." };
  } catch (e) {
    return { error: e instanceof Error ? e.message : String(e) };
  }
}

/** The value at `path` (`["color", "bg"]`), or undefined. */
export function tokenAt(tokens: Tokens | null, path: readonly string[]): unknown {
  let v: unknown = tokens;
  for (const key of path) {
    if (!isObject(v)) return undefined;
    v = v[key];
  }
  return v;
}

/** A copy of `tokens` with `path` set to `value` (or removed when it is
 *  undefined). Missing objects on the way are made; key order is kept, so
 *  the file reads as it did. */
export function setToken(tokens: Tokens, path: readonly string[], value: unknown): Tokens {
  const [key, ...rest] = path;
  if (key === undefined) return tokens;
  const next: Tokens = { ...tokens };
  if (rest.length === 0) {
    if (value === undefined) delete next[key];
    else next[key] = value;
    return next;
  }
  const child = isObject(tokens[key]) ? (tokens[key] as Tokens) : {};
  next[key] = setToken(child, rest, value);
  return next;
}

/** The names under a section such as `color` or `motion.spring`, in file
 *  order. */
export function sectionKeys(tokens: Tokens | null, section: string): string[] {
  const v = tokenAt(tokens, section.split("."));
  return isObject(v) ? Object.keys(v) : [];
}

/** The text a draft is saved as: two-space JSON and a final newline, as the
 *  starter themes are written. */
export function formatTheme(tokens: Tokens): string {
  return `${JSON.stringify(tokens, null, 2)}\n`;
}

/** Two drafts hold the same tokens in the same order. */
export function sameTokens(a: Tokens | null, b: Tokens | null): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}

/** A theme name: lowercase letters, digits and `-`, at most 64 characters
 *  (`studio_core::valid_theme_name`). */
export function validThemeName(name: string): boolean {
  return /^[a-z0-9-]{1,64}$/.test(name);
}

/** `<base>-copy`, then `-copy-2`, `-copy-3`... until no name in `taken`
 *  has it. A base that is not a theme name is made into one first. */
export function copyName(base: string, taken: Iterable<string>): string {
  const used = new Set(taken);
  const stem =
    base
      .toLowerCase()
      .replace(/[^a-z0-9-]+/g, "-")
      .replace(/^-+|-+$/g, "")
      .slice(0, 50) || "theme";
  const first = `${stem}-copy`;
  if (!used.has(first)) return first;
  for (let n = 2; ; n++) {
    const name = `${first}-${n}`;
    if (!used.has(name)) return name;
  }
}

/** Which role a type entry's name suggests: headings and display sizes take
 *  the heading font, `code` the mono one, the rest the body font. */
export function typeRole(name: string): FontRole {
  if (/^(display|h\d|title|heading)/i.test(name)) return "heading";
  if (/^(code|mono)/i.test(name)) return "mono";
  return "body";
}

/**
 * Sets `font.<role>` to `family` and moves the type entries that used the
 * role's old family with it, since components spread `type.*` and never read
 * `font` for those. When two roles share the old family, the entry's name
 * decides which role it belongs to (`typeRole`).
 */
export function setFontRole(tokens: Tokens, role: FontRole, family: string): Tokens {
  const old = tokenAt(tokens, ["font", role]);
  let next = setToken(tokens, ["font", role], family);
  if (typeof old !== "string" || old === family) return next;
  const shared = (["heading", "body", "mono"] as const).some(
    (r) => r !== role && tokenAt(tokens, ["font", r]) === old,
  );
  for (const name of sectionKeys(tokens, "type")) {
    if (tokenAt(tokens, ["type", name, "fontFamily"]) !== old) continue;
    if (shared && typeRole(name) !== role) continue;
    next = setToken(next, ["type", name, "fontFamily"], family);
  }
  return next;
}

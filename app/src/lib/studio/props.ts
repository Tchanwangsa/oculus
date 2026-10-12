import type { PropSpec } from "@/lib/studio/components";

// What the Components inspector needs to know about a prop's schema
// (`PropSpec` in components.ts): the names a token prop may take, a blank
// value to add to a list, and how to show an option.

/** The props being drawn: the keys the inspector has set. */
export type PropValues = Record<string, unknown>;

/** The section of a theme's tokens a path names (`color`, or `motion.spring`
 *  for a nested one); null when the theme lacks it or it holds no names. */
export function tokenSection(
  tokens: Record<string, unknown> | null,
  path: string,
): Record<string, unknown> | null {
  let at: unknown = tokens;
  for (const key of path.split(".")) {
    if (at == null || typeof at !== "object" || Array.isArray(at)) return null;
    at = (at as Record<string, unknown>)[key];
  }
  return at != null && typeof at === "object" && !Array.isArray(at) ? (at as Record<string, unknown>) : null;
}

/** The value a new list row starts with. */
export function blankValue(spec: PropSpec, tokens: Record<string, unknown> | null): unknown {
  switch (spec.kind) {
    case "num":
      return 0;
    case "bool":
      return false;
    case "list":
      return [];
    case "oneOf":
      return spec.values[0] ?? "";
    case "token":
      return Object.keys(tokenSection(tokens, spec.token) ?? {})[0] ?? "";
    default:
      return "";
  }
}

/** Whether a prop can go back to not being set: it has a default to fall to. */
export function unsettable(spec: PropSpec): boolean {
  return spec.optional || spec.default !== undefined;
}

/** An option of a `oneOf` as the words to show. */
export function optionText(value: unknown): string {
  return typeof value === "string" ? value : JSON.stringify(value);
}

/** A theme token that is a plain CSS colour, for the dot beside its name. */
export function cssColor(value: unknown): string | null {
  return typeof value === "string" && /^(#[0-9a-f]{3,8}|rgba?\(.*\)|hsla?\(.*\))$/i.test(value) ? value : null;
}

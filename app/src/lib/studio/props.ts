import type { NamedProp, PropSpec } from "@/lib/studio/components";

// What the Components inspector needs to know about a prop's schema
// (`PropSpec` in components.ts): the names a token prop may take, a blank
// value to add to a list, how to show an option, how a prop is described,
// whether it was changed, and which section of the form it belongs in.

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

const PLURAL: Record<PropSpec["kind"], string> = {
  str: "texts",
  latex: "LaTeX strings",
  num: "numbers",
  bool: "switches",
  list: "lists",
  oneOf: "choices",
  token: "theme names",
};

/** What a prop takes, in words: "Number", "List of numbers", "One of a, b". */
export function describeKind(spec: PropSpec): string {
  switch (spec.kind) {
    case "str":
      return "Text";
    case "latex":
      return "LaTeX";
    case "num":
      return "Number";
    case "bool":
      return "On or off";
    case "oneOf":
      return `One of ${spec.values.map(optionText).join(", ")}`;
    case "token":
      return `A name in ${spec.token}`;
    case "list":
      return `List of ${PLURAL[spec.of.kind]}`;
  }
}

/** The tooltip text of a prop: its kind, then its default, or that it is
 *  optional or required. */
export function describeProp(spec: PropSpec): string {
  const rest =
    spec.default !== undefined
      ? `default ${JSON.stringify(spec.default)}`
      : spec.optional
        ? "optional"
        : "required";
  return `${describeKind(spec)} · ${rest}`;
}

/** Two prop values hold the same JSON. */
export const sameValue = (a: unknown, b: unknown): boolean => JSON.stringify(a) === JSON.stringify(b);

/** A prop the inspector has set to something other than what it would take
 *  unset. A required prop has no unset state, so it is never "changed". */
export function changed(spec: NamedProp, values: PropValues): boolean {
  return spec.name in values && unsettable(spec) && !sameValue(values[spec.name], spec.default);
}

/** A named section of the props form. */
export interface PropGroup {
  id: "content" | "style" | "timing" | "props";
  title: string;
  props: NamedProp[];
}

/** Names of moments and spans: `at`, `plotAt`, `revealEach`, `zoomDur`. */
const TIMING = /^(at|each|delay|dur|duration|stagger)$|(At|Each|Dur|Delay|Duration)$/;

/** Looks rather than data: colours from the theme, a closed set of options,
 *  or an on/off switch, alone or listed. */
function isStyle(spec: PropSpec): boolean {
  if (spec.kind === "list") return isStyle(spec.of);
  return spec.kind === "token" || spec.kind === "oneOf" || spec.kind === "bool";
}

const GROUP_TITLES: Record<"content" | "style" | "timing", string> = {
  content: "Content",
  style: "Style",
  timing: "Timing",
};

/** Few props, or props that all fall in one group, stay as one list. */
const SPLIT_FROM = 6;

/**
 * The props split into Content, Style and Timing by the schema and the
 * names, in the order the component declares them: a name that reads as a
 * moment (`at`, `markAt`) is timing; a colour, option or switch is style;
 * the rest is content. A form with few props, or with only one non-empty
 * group, is a single "Props" group.
 */
export function groupProps(props: NamedProp[]): PropGroup[] {
  const whole: PropGroup[] = [{ id: "props", title: "Props", props }];
  if (props.length < SPLIT_FROM) return whole;
  const buckets: Record<"content" | "style" | "timing", NamedProp[]> = { content: [], style: [], timing: [] };
  for (const prop of props) {
    buckets[TIMING.test(prop.name) ? "timing" : isStyle(prop) ? "style" : "content"].push(prop);
  }
  const groups = (["content", "style", "timing"] as const)
    .filter((id) => buckets[id].length > 0)
    .map((id) => ({ id, title: GROUP_TITLES[id], props: buckets[id] }));
  return groups.length < 2 ? whole : groups;
}

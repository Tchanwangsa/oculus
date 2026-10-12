import { invoke } from "@tauri-apps/api/core";

// The Components playground (`app/src-tauri/src/studio/commands/components/`):
// the components a video could use, one component's source and frames, and
// edits that must load, with a history to undo them from. A name is a
// PascalCase identifier; `subject` is a course code, and without one only
// the global library is read.

/** Which library a component lives in; the subject's replaces the global
 *  one of the same name. */
export type ComponentLevel = "subject" | "global";

/** A prop builder's own JSON: `str()`, `num()`, `bool()`, `list(x)`,
 *  `oneOf([...])`, `token(kind)` or `latex()`. */
export type PropSpec = {
  optional: boolean;
  default?: unknown;
} & (
  | { kind: "str" | "num" | "bool" | "latex" }
  | { kind: "list"; of: PropSpec }
  | { kind: "oneOf"; values: unknown[] }
  /** `token` is a theme section such as `color` or `motion.spring`; the
   *  prop's value is a name in it. */
  | { kind: "token"; token: string }
);

export type NamedProp = PropSpec & { name: string };

export interface ComponentInfo {
  name: string;
  level: ComponentLevel;
  /** A shell lays out named areas (its name ends in `Shell`); anything else
   *  is a component. */
  kind: "shell" | "component";
  /** In the order the component declares them; empty when it does not load. */
  props: NamedProp[];
  /** Named example props, in declaration order. */
  examples: Record<string, Record<string, unknown>>;
  /** Why the component does not load. */
  problem: string | null;
  /** A subject's file that replaces a global component: its "own version". */
  shadows: boolean;
  /** The name is one of studio-core's starter set, whichever level holds the
   *  file now and whether it was edited. */
  starter: boolean;
}

export interface ComponentText {
  level: ComponentLevel;
  text: string;
}

/** One earlier text of a component, kept when it was replaced. */
export interface Snapshot {
  /** What `restoreComponent` takes. */
  id: string;
  /** When the text was replaced, in Unix milliseconds. */
  savedAt: number;
}

/** The components over the cascade, one row per name from the nearest level,
 *  sorted by name. */
export function listComponents(subject?: string | null): Promise<ComponentInfo[]> {
  return invoke<ComponentInfo[]>("studio_components", { subject: subject ?? null });
}

/** The source from the nearest level that has the component. */
export function componentSource(name: string, subject?: string | null): Promise<ComponentText> {
  return invoke<ComponentText>("studio_component_source", { name, subject: subject ?? null });
}

export interface ComponentFrameOptions {
  subject?: string | null;
  /** A theme name, resolved through the same levels. */
  theme: string;
  /** A theme file's text to draw in instead of the saved `theme` (an
   *  unsaved edit); `theme` then only names it in errors. */
  themeText?: string | null;
  /** The props to draw; else `example`'s; else the first example's. */
  props?: Record<string, unknown>;
  example?: string;
  /** Seconds into the component, from 0 and before `duration`. */
  t: number;
  /** PNG width in px, 1 to 4096; the frame is 16:9. */
  width: number;
  /** The scene length the component sees as `ctx.duration` (default 4 s). */
  duration?: number;
}

/** A PNG of the component. Rejects with a sentence fit to show: props the
 *  schema refuses, a theme that does not parse, an error in the TSX. */
export async function componentFrame(name: string, options: ComponentFrameOptions): Promise<Blob> {
  const bytes = await invoke<ArrayBuffer>("studio_component_frame", {
    name,
    subject: options.subject ?? null,
    theme: options.theme,
    themeText: options.themeText ?? null,
    props: options.props ?? null,
    example: options.example ?? null,
    t: options.t,
    width: options.width,
    duration: options.duration ?? null,
  });
  return new Blob([bytes], { type: "image/png" });
}

/** Writes `text` as the component at `level` (with the subject's code for
 *  the subject level). Rejects, writing nothing, when the text does not
 *  load; the text it replaces goes to the history. Resolves to the text
 *  written. */
export function writeComponent(
  name: string,
  level: ComponentLevel,
  subject: string | null | undefined,
  text: string,
): Promise<ComponentText> {
  return invoke<ComponentText>("studio_component_write", {
    name,
    level,
    subject: subject ?? null,
    text,
  });
}

/** The component's earlier texts at `level`, newest first. */
export function componentHistory(
  name: string,
  level: ComponentLevel,
  subject?: string | null,
): Promise<Snapshot[]> {
  return invoke<Snapshot[]>("studio_component_history", {
    name,
    level,
    subject: subject ?? null,
  });
}

/** Writes snapshot `id` back. The text it replaces becomes the newest
 *  snapshot, so a restore can itself be undone. */
export function restoreComponent(
  name: string,
  level: ComponentLevel,
  subject: string | null | undefined,
  id: string,
): Promise<ComponentText> {
  return invoke<ComponentText>("studio_component_restore", {
    name,
    level,
    subject: subject ?? null,
    id,
  });
}

/** Removes the subject's own version of a component, so the subject uses the
 *  global one again. Its text goes to the history first. */
export function deleteComponent(name: string, subject: string): Promise<void> {
  return invoke<void>("studio_component_delete", { name, subject });
}

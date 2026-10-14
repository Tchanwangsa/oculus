import { isGroupBy, type GroupBy } from "../tasks/taskGroups";
import { isSortKey, type TaskSort } from "../tasks/taskSort";

/**
 * How the Tasks page draws its list: one object, kept as JSON in localStorage
 * (`oculus-tasks-display-v2`) and read back field by field, so an old or edited
 * value costs only the field it spoils. The key is versioned because a stored
 * object overrides every default: a changed default reaches existing installs
 * only under a new key.
 */

export const DISPLAY_KEY = "oculus-tasks-display-v2";

/** Every table column but Name, in the order the panel lists hidden ones. */
export const PROPERTIES = ["status", "tags", "project", "subject", "due", "updated", "created"] as const;

export type PropertyId = (typeof PROPERTIES)[number];

export const PROPERTY_LABELS: Record<PropertyId, string> = {
  status: "Status",
  tags: "Tags",
  project: "Project",
  subject: "Subject",
  due: "Due",
  updated: "Updated",
  created: "Created",
};

export type TaskLayout = "table" | "board";

export interface TasksDisplay {
  view: TaskLayout;
  /** Table only. */
  group: GroupBy;
  sort: TaskSort;
  showCompleted: boolean;
  /** The shown columns after Name, left to right. Name is always first and
   *  is not in the list; every id missing from it is hidden. */
  properties: PropertyId[];
}

export const DEFAULT_DISPLAY: TasksDisplay = {
  view: "table",
  group: "none",
  sort: { key: "due", dir: "asc" },
  showCompleted: false,
  properties: ["status", "tags", "project", "due", "updated"],
};

export function isPropertyId(value: unknown): value is PropertyId {
  return (PROPERTIES as readonly unknown[]).includes(value);
}

function clone(display: TasksDisplay): TasksDisplay {
  return { ...display, sort: { ...display.sort }, properties: [...display.properties] };
}

/** Shown ids that exist, once each; `null` when `value` is not an array. A
 *  future property is therefore hidden until the user shows it. */
function readProperties(value: unknown): PropertyId[] | null {
  if (!Array.isArray(value)) return null;
  const seen = new Set<PropertyId>();
  for (const item of value) if (isPropertyId(item)) seen.add(item);
  return [...seen];
}

/** The stored JSON, or the default for anything unreadable, field by field. */
export function readDisplay(raw: string | null): TasksDisplay {
  let stored: Record<string, unknown> = {};
  try {
    const parsed: unknown = raw == null ? null : JSON.parse(raw);
    if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
      stored = parsed as Record<string, unknown>;
    }
  } catch {
    // Malformed JSON: every field takes its default.
  }
  const sort = stored.sort as Record<string, unknown> | null | undefined;
  return {
    view: stored.view === "board" || stored.view === "table" ? stored.view : DEFAULT_DISPLAY.view,
    group: isGroupBy(stored.group) ? stored.group : DEFAULT_DISPLAY.group,
    sort: {
      key: isSortKey(sort?.key) ? sort.key : DEFAULT_DISPLAY.sort.key,
      dir: sort?.dir === "asc" || sort?.dir === "desc" ? sort.dir : DEFAULT_DISPLAY.sort.dir,
    },
    showCompleted:
      typeof stored.showCompleted === "boolean" ? stored.showCompleted : DEFAULT_DISPLAY.showCompleted,
    properties: readProperties(stored.properties) ?? [...DEFAULT_DISPLAY.properties],
  };
}

export function writeDisplay(display: TasksDisplay): string {
  return JSON.stringify(display);
}

export function resetDisplay(): TasksDisplay {
  return clone(DEFAULT_DISPLAY);
}

export function isDefaultDisplay(display: TasksDisplay): boolean {
  return writeDisplay(display) === writeDisplay(DEFAULT_DISPLAY);
}

/** The properties not shown, in {@link PROPERTIES} order. */
export function hiddenProperties(display: TasksDisplay): PropertyId[] {
  return PROPERTIES.filter((p) => !display.properties.includes(p));
}

export function showProperty(display: TasksDisplay, id: PropertyId): TasksDisplay {
  if (display.properties.includes(id)) return display;
  return { ...display, properties: [...display.properties, id] };
}

export function hideProperty(display: TasksDisplay, id: PropertyId): TasksDisplay {
  return { ...display, properties: display.properties.filter((p) => p !== id) };
}

export function showAllProperties(display: TasksDisplay): TasksDisplay {
  return { ...display, properties: [...display.properties, ...hiddenProperties(display)] };
}

export function hideAllProperties(display: TasksDisplay): TasksDisplay {
  return { ...display, properties: [] };
}

/** Moves `id` by `by` places within the shown list; the same display when it
 *  would leave the list. */
export function moveProperty(display: TasksDisplay, id: PropertyId, by: -1 | 1): TasksDisplay {
  const from = display.properties.indexOf(id);
  const to = from + by;
  if (from === -1 || to < 0 || to >= display.properties.length) return display;
  const next = [...display.properties];
  next.splice(to, 0, next.splice(from, 1)[0]);
  return { ...display, properties: next };
}

/** A click on a column header: the same key flips direction, a new key starts
 *  ascending. */
export function sortByHeader(sort: TaskSort, key: TaskSort["key"]): TaskSort {
  return sort.key === key ? { key, dir: sort.dir === "asc" ? "desc" : "asc" } : { key, dir: "asc" };
}

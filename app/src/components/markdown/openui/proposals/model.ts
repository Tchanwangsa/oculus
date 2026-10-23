import type { ProposalState } from "@/lib/harness/proposals";

export const PROPOSAL_KINDS = new Set(["NewTasks", "NewProject", "NewEvents", "NewNote", "TaskChanges", "ProjectChanges", "EventChanges", "TagChanges", "DeleteTasks", "DeleteEvents"]);
export const JOB_KINDS = new Set(["Sync", "Reparse", "Transcribe", "LectureChapters", "LectureEnd", "Download"]);
export type Row = Record<string, unknown>;
export const object = (value: unknown): Row => value && typeof value === "object" && !Array.isArray(value) ? value as Row : {};
export const rows = (value: unknown): Row[] => Array.isArray(value) ? value.map(object) : [];

/** Runtime elements carry rendering metadata; Rust receives only the named
 * signature fields, including nested Task/Event/Change contents. */
export function plain(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(plain);
  if (!value || typeof value !== "object") return value;
  const row = object(value);
  if (row.type === "element") return plain(row.props);
  return Object.fromEntries(Object.entries(row).map(([key, entry]) => [key, plain(entry)]));
}

export function initialProposal(kind: string, input: Row): ProposalState {
  const state = object(plain(input));
  if (kind === "DeleteTasks" || kind === "DeleteEvents") {
    const key = kind === "DeleteTasks" ? "tasks" : "events";
    state[key] = (state[key] as unknown[]).map((id) => ({ id, included: true }));
  }
  return state;
}

export function rowKey(kind: string): string | null {
  if (kind === "NewTasks") return "items";
  if (kind === "NewProject" || kind === "DeleteTasks") return "tasks";
  if (kind === "NewEvents" || kind === "DeleteEvents") return "events";
  if (kind.endsWith("Changes")) return "changes";
  return null;
}

export const entityFor = (kind: string) => kind.includes("Project") ? "project" : kind.includes("Event") ? "event" : kind.includes("Tag") ? "tag" : "task";
export const rowId = (row: Row) => row.id ?? row.task ?? row.tag;
export const included = (row: Row) => row.included !== false;

export function equal(a: unknown, b: unknown): boolean {
  if (Array.isArray(a) && Array.isArray(b)) return a.length === b.length && a.every((v, i) => equal(v, b[i]));
  if (a && b && typeof a === "object" && typeof b === "object") {
    const aa = object(a), bb = object(b), keys = Object.keys(aa);
    return keys.length === Object.keys(bb).length && keys.every((key) => equal(aa[key], bb[key]));
  }
  return (a ?? null) === (b ?? null);
}

export function conflicts(row: Row, live: Row | null | undefined): string[] {
  if (!row.from) return [];
  return Object.keys(object(row.from)).filter((key) => !live || !equal(object(row.from)[key], live[key]));
}

export function includeChange(row: Row, checked: boolean, live: Row | null | undefined): Row {
  const keys = Object.keys(object(row.from));
  return { ...row, included: checked, ...(checked && live && conflicts(row, live).length ? { from: Object.fromEntries(keys.map((key) => [key, live[key] ?? null])) } : {}) };
}

export function effectiveChanges(row: Row): Row {
  const from = object(row.from);
  return Object.fromEntries(Object.entries(object(row.to)).filter(([key, value]) => !equal(from[key], value)));
}

const KEYS: Record<string, string[]> = {
  TaskChanges: ["title", "body", "due", "starts", "estimate", "tags", "files", "column", "project"],
  ProjectChanges: ["name", "subject", "due", "starts", "brief", "tags", "event", "status"],
  EventChanges: ["title", "start", "end", "kind", "subject", "notes"],
  TagChanges: ["name", "color"],
};
const DATE = /^\d{4}-(?:0[1-9]|1[0-2])-(?:0[1-9]|[12]\d|3[01])(?:[Tt ](?:[01]\d|2[0-3]):[0-5]\d(?::(?:[0-5]\d|60))?(?:\.\d+)?(?:[Zz]|[+-](?:[01]\d|2[0-3]):?[0-5]\d)?)?$/;
const fail = (message: string): never => { throw new Error(message); };
const numericId = (v: unknown) => typeof v === "number" && Number.isInteger(v) && v > 0;
const eventId = (v: unknown) => typeof v === "string" && /^local_[1-9]\d*$/.test(v);
const date = (v: unknown) => v == null || (typeof v === "string" && DATE.test(v.trim()));
const strings = (v: unknown) => v == null || (Array.isArray(v) && v.every((x) => typeof x === "string"));

function checkTask(task: Row, depth = 0) {
  if (typeof task.title !== "string" || !task.title.trim()) fail("A task needs a title");
  if (!date(task.due) || !date(task.starts)) fail("Task dates must be ISO 8601");
  if (task.estimate != null && (!Number.isInteger(task.estimate) || Number(task.estimate) < 0)) fail("Estimate must be whole minutes");
  const children = rows(task.subtasks);
  if (depth && children.length) fail("Subtasks may nest only one level");
  children.forEach((child) => checkTask(child, depth + 1));
}

/** Checks constraints which the positional component schema cannot express.
 * Called before rendering the fence and again for edited state on Apply. */
export function validateProposal(kind: string, state: Row): void {
  const array = rowKey(kind);
  if (array && !Array.isArray(state[array])) fail(`${kind} requires ${array}`);
  if (kind === "NewTasks") {
    if (!("project" in state) || state.project === undefined) fail("NewTasks requires project; use null for unfiled");
    if (state.project != null && !numericId(state.project)) fail("Invalid project id");
    rows(state.items).forEach((row) => {
      if (row.task != null) {
        if (!numericId(row.task)) fail("Invalid parent task id");
        rows(row.subtasks).forEach((task) => checkTask(task, 1));
      } else checkTask(row);
    });
  }
  if (kind === "NewProject") {
    if (!("subject" in state) || state.subject === undefined || (state.subject != null && typeof state.subject !== "string")) fail("NewProject requires subject; use null for personal");
    if (typeof state.name !== "string" || !state.name.trim()) fail("A project needs a name");
    if (!date(state.due) || !date(state.starts)) fail("Project dates must be ISO 8601");
    rows(state.tasks).forEach((task) => checkTask(task));
  }
  if (kind === "NewNote" && ["subject", "title", "body"].some((key) => typeof state[key] !== "string" || !String(state[key]).trim())) fail("NewNote requires subject, title and body");
  if (kind === "NewEvents") rows(state.events).forEach((event) => {
    if (typeof event.title !== "string" || !event.title.trim()) fail("An event needs a title");
    if (!event.start || !date(event.start) || !date(event.end)) fail("Event dates must be ISO 8601");
    if (event.end != null && event.kind !== "class") fail("Only class events may have an end");
  });
  if (kind === "DeleteEvents" || kind === "DeleteTasks") {
    const key = kind === "DeleteTasks" ? "tasks" : "events";
    for (const value of state[key] as unknown[]) {
      const id = typeof value === "object" ? object(value).id : value;
      if (!(kind === "DeleteTasks" ? numericId(id) : eventId(id))) fail("Invalid deletion id; only local_ events can be deleted");
    }
  }
  if (kind.endsWith("Changes")) rows(state.changes).forEach((change) => {
    if (kind === "TagChanges" && !change.from) return;
    if (!(kind === "EventChanges" ? eventId(change.id) : numericId(change.id))) fail("Invalid change id; only local_ events can be changed");
    const from = object(change.from), to = object(change.to), keys = Object.keys(to);
    if (!keys.length || !equal([...Object.keys(from)].sort(), [...keys].sort())) fail("Change from and to must hold the same keys");
    for (const key of keys) {
      if (!KEYS[kind]?.includes(key)) fail(`Unknown ${kind} field: ${key}`);
      for (const value of [from[key], to[key]]) {
        if (["due", "starts", "start", "end"].includes(key) && !date(value)) fail("Changes require ISO 8601 dates");
        if (["tags", "files"].includes(key) && !strings(value)) fail(`${key} must be a string array`);
        if (key === "estimate" && value != null && (!Number.isInteger(value) || Number(value) < 0)) fail("Estimate must be whole minutes");
        if (key === "project" && value != null && !numericId(value)) fail("Project must be a numeric id or null");
        if (!["tags", "files", "estimate", "project"].includes(key) && value != null && typeof value !== "string") fail(`${key} must be text or null`);
        if (key === "kind" && value != null && !["note", "due", "class"].includes(String(value))) fail("Unknown event kind");
        if (key === "status" && !["active", "archived"].includes(String(value))) fail("Unknown project status");
        if (key === "color" && !["slate", "indigo", "green", "amber", "rose", "purple", "teal", "orange"].includes(String(value))) fail("Unknown tag colour");
      }
    }
    if (kind === "EventChanges" && to.end != null && to.kind != null && to.kind !== "class") fail("Only class events may have an end");
  });
}

export function validateCardTree(root: unknown): void {
  let count = 0;
  const visit = (node: unknown) => {
    if (Array.isArray(node)) { node.forEach(visit); return; }
    const item = object(node);
    if (item.type === "element") {
      const kind = String(item.typeName);
      const props = object(item.props);
      if (kind === "Value" && !("value" in props)) fail("Value requires value; use null for an undefined result");
      if (kind === "Chart" && Array.isArray(props.points) && props.points.some((point) => !("y" in object(point)))) fail("Chart points require y; use null for an undefined result");
      if (PROPOSAL_KINDS.has(kind) || JOB_KINDS.has(kind)) {
        if (++count > 1) fail("Use a separate openui fence for each proposal or job");
        validateProposal(kind, object(plain(item.props)));
      }
      visit(item.props);
    } else Object.values(item).forEach((value) => { if (value && typeof value === "object") visit(value); });
  };
  visit(root);
}

/**
 * Automations: graphs of nodes joined by links that carry values.
 *
 * A trigger fires and produces named outputs — a sync-complete trigger hands
 * out the run's new, updated and changed file lists. Every link is both an
 * order ("run this after that") and a wire ("with this value in that slot"),
 * so what a node works on is drawn on the canvas rather than baked into its
 * kind. An "Ask AI" node summarises whatever files you wired into it; an
 * "Add to Inbox" node writes whatever you wired into it.
 *
 * Nothing here knows what a digest is. "Summarise new files after a sync" is
 * one drawing of four nodes, not a node.
 *
 * The graph is JSON on the automation row, so adding a node kind never needs
 * a migration. Firing state is not part of that document: per-trigger anchors
 * live in the `trigger_state` column, because the scheduler queries them every
 * 30s and because rewriting the user's drawing on every fire would be wrong.
 */

import { invoke } from "@tauri-apps/api/core";
import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from "@tauri-apps/plugin-notification";
import { triggerSync } from "@/lib/syncRunner";
import { humanizeSlug, sqliteUtcToMs } from "@/lib/format";
import { deliverSummaries, materialiseSummaries } from "@/lib/digest";
import {
  addInboxNote,
  addLocalEvent,
  getAllLectures,
  getAutomations,
  getCalendarEvents,
  getFilesForAutomation,
  getInboxDigest,
  getLatestSyncRunId,
  getLocalEvents,
  getSyncRunFiles,
  markAutomationFired,
  setAutomationTriggerState,
  updateAutomation,
  type DbAutomation,
} from "@/lib/db";
import { useInboxStore } from "@/stores/inboxStore";

export type NodeKind =
  | "trigger.schedule"
  | "trigger.event"
  | "condition.if"
  | "source.inbox"
  | "source.calendar"
  | "source.files"
  | "action.sync"
  | "action.summarise"
  | "action.ai"
  | "action.inbox"
  | "action.calendar"
  | "action.notify";

export interface AutomationNode {
  id: string;
  kind: NodeKind;
  config: Record<string, any>;
  /** Canvas coordinates. Absent on graphs written before the editor existed;
   *  the editor lays those out on first open and saves positions back. */
  position?: { x: number; y: number };
}

/** A wire from one node's output port into another node's input slot. */
export interface AutomationLink {
  from: string;
  fromPort: string;
  to: string;
  toPort: string;
}

export interface AutomationGraph {
  nodes: AutomationNode[];
  links: AutomationLink[];
}

export const EMPTY_GRAPH: AutomationGraph = { nodes: [], links: [] };

export const isTrigger = (k: NodeKind) => k.startsWith("trigger.");
export const isCondition = (k: NodeKind) => k.startsWith("condition.");
/** A source reads something already in the database. It is deliberately *not*
 *  a trigger: nothing decides on its own when "what is due this week" is worth
 *  asking, so it waits for a signal wired into it. */
export const isSource = (k: NodeKind) => k.startsWith("source.");

// ── Ports ────────────────────────────────────────────────────────────────────
//
// A node's ports are derived from its kind and config, never stored: the event
// trigger's outputs depend on which event it listens for, and an AI node's
// input slots are whatever the user named. Storing them would mean two copies
// of the same fact, one of them stale.

export type PortType = "signal" | "files" | "summaries" | "text" | "number" | "any";

export interface Port {
  id: string;
  label: string;
  type: PortType;
}

/** What anything that runs a sync hands downstream. */
const FILE_OUTPUTS: Port[] = [
  { id: "new", label: "New files", type: "files" },
  { id: "updated", label: "Updated files", type: "files" },
  { id: "changed", label: "New or updated", type: "files" },
];

/** The AI node's input slots are user-defined — that is the whole point of it:
 *  you name the things your prompt talks about and wire them up. An explicit
 *  empty list means a prompt that needs no input. */
export function aiSlots(config: Record<string, any> | undefined): Port[] {
  const raw = config?.slots;
  if (!Array.isArray(raw)) return [{ id: "input", label: "Input", type: "any" }];
  return raw
    .filter((s) => s && typeof s.id === "string" && s.id)
    .map((s) => ({ id: s.id, label: s.label || s.id, type: "any" as PortType }));
}

export function outputPorts(n: AutomationNode): Port[] {
  switch (n.kind) {
    case "trigger.schedule":
      return [{ id: "then", label: "Fires", type: "signal" }];
    case "trigger.event":
      return n.config?.event === "sync-complete"
        ? FILE_OUTPUTS
        : [{ id: "then", label: "Fires", type: "signal" }];
    case "condition.if": {
      // One port per branch, in the order they are tested, so the canvas
      // reads top to bottom the way the router runs.
      const ports: Port[] = branchesOf(n).map((b) => ({
        id: b.id,
        label: b.label || "Branch",
        type: "any",
      }));
      if (n.config?.otherwise !== false) {
        ports.push({ id: "else", label: "Otherwise", type: "any" });
      }
      return ports;
    }
    case "source.inbox":
      return [
        { id: "items", label: "Items", type: "text" },
        { id: "count", label: "How many", type: "number" },
      ];
    case "source.calendar":
      return [
        { id: "events", label: "Events", type: "text" },
        { id: "count", label: "How many", type: "number" },
      ];
    case "source.files":
      return [
        { id: "files", label: "Files", type: "files" },
        { id: "count", label: "How many", type: "number" },
      ];
    case "action.sync":
      return FILE_OUTPUTS;
    case "action.summarise":
      return [{ id: "summaries", label: "Summaries", type: "summaries" }];
    case "action.ai":
      return [{ id: "text", label: "Reply", type: "text" }];
    case "action.inbox":
    case "action.calendar":
    case "action.notify":
      return [{ id: "then", label: "Done", type: "signal" }];
  }
}

export function inputPorts(n: AutomationNode): Port[] {
  switch (n.kind) {
    case "trigger.schedule":
    case "trigger.event":
      return [];
    case "condition.if":
      return [{ id: "input", label: "Value", type: "any" }];
    case "source.inbox":
    case "source.calendar":
    case "source.files":
      return [{ id: "then", label: "Run", type: "signal" }];
    case "action.sync":
      return [{ id: "then", label: "Run", type: "signal" }];
    case "action.summarise":
      return [{ id: "files", label: "Files", type: "files" }];
    case "action.ai":
      return aiSlots(n.config);
    case "action.inbox":
    case "action.calendar":
    case "action.notify":
      return [{ id: "input", label: "Input", type: "any" }];
  }
}

export const portById = (ports: Port[], id: string): Port | undefined =>
  ports.find((p) => p.id === id);

/**
 * Whether a wire may be drawn. Only the file slots are fussy: everything can
 * be read as text or used as a bare "run after this", but a list of files
 * cannot be conjured from a sentence.
 *
 * `any` is the exception, because it does not mean "anything at all" — it is
 * what a pass-through port carries, and the only ports typed `any` on the
 * output side are a condition's branches, which emit whatever was wired into
 * the condition. What that is cannot be known until the graph runs, so
 * refusing it would ban the most natural drawing there is: gate on how many
 * files arrived, then summarise them. A branch fed something that is not a
 * file list summarises nothing, which is a quiet no-op rather than a crash.
 */
export function canConnect(from: PortType, to: PortType): boolean {
  if (to === "files") return from === "files" || from === "summaries" || from === "any";
  if (to === "summaries") return from === "summaries" || from === "any";
  return true;
}

/** The port a link lands on when the shape is implied rather than drawn —
 *  graph upgrades, and the palette's "connect it for me" paths. */
const defaultOutPort = (n: AutomationNode): string => {
  const ports = outputPorts(n);
  return (portById(ports, "changed") ?? ports[0])?.id ?? "then";
};
const defaultInPort = (n: AutomationNode): string => inputPorts(n)[0]?.id ?? "input";

// ── Conditions ───────────────────────────────────────────────────────────────
//
// A condition routes, it does not fan out: branches are tested top to bottom
// and the *first* match takes the value, so "more than ten files" and "any
// files at all" can live in one node without both firing. Whatever matches
// nothing leaves by `else`.
//
// Both sides of a rule are operands rather than a field and a literal, because
// half the questions worth asking compare two facts ("the hour is past the
// deadline hour"), not a fact and a number.

export type OperandKind = "input" | "clock" | "number" | "text";

export interface Operand {
  kind: OperandKind;
  /** kind "input": which aspect of the wired-in value. */
  field?: "count" | "text" | "names";
  /** kind "clock": which fact about now. */
  clock?: "hour" | "minute" | "weekday" | "day" | "month";
  n?: number;
  text?: string;
}

export type RuleOp =
  | "gt" | "gte" | "lt" | "lte" | "eq" | "ne"
  | "contains" | "notContains" | "startsWith" | "endsWith" | "matches"
  | "empty" | "notEmpty";

export interface Rule {
  left: Operand;
  op: RuleOp;
  right: Operand;
}

export interface Branch {
  id: string;
  label: string;
  match: "all" | "any";
  /** No rules is "always" — the last branch of a router is often just a name
   *  for "everything else that got this far". */
  rules: Rule[];
}

/** Ops that ask about the left side alone, so the inspector knows to hide the
 *  right-hand operand rather than render a control nothing reads. */
export const UNARY_OPS: RuleOp[] = ["empty", "notEmpty"];

export const isUnaryOp = (op: RuleOp): boolean => UNARY_OPS.includes(op);

/** Branch ids are port ids, so they must survive every edit around them —
 *  a positional id would silently move every wire when a branch is deleted. */
/** Branch ids are port ids, so two branches sharing one would be two ports
 *  sharing one. A timestamp alone collides when two are added in the same
 *  millisecond, which a template or a paste can do. */
export const newBranchId = (): string =>
  `b${Date.now().toString(36)}${Math.floor(Math.random() * 1296).toString(36).padStart(2, "0")}`;

export const newBranch = (label = "Branch"): Branch => ({
  id: newBranchId(),
  label,
  match: "all",
  rules: [],
});

/** The branches as stored, with every gap filled in: a graph edited by hand,
 *  or written by a version that knew fewer fields, still has to yield ports. */
export function branchesOf(n: AutomationNode): Branch[] {
  const raw = n.config?.branches;
  if (!Array.isArray(raw)) return [];
  return raw
    .filter((b) => b && typeof b.id === "string" && b.id)
    .map((b) => ({
      id: b.id,
      label: typeof b.label === "string" ? b.label : b.id,
      match: b.match === "any" ? "any" : "all",
      rules: Array.isArray(b.rules) ? (b.rules as Rule[]) : [],
    }));
}

/** One operand as a plain JS value. `input` reads the wired-in value with
 *  `plainText`, never `valueToText`: asking a question must never be what
 *  spends tokens, or a graph would pay for summaries just to decide whether it
 *  wants them. */
function operandValue(o: Operand | undefined, input: Value | undefined, now: Date): string | number {
  switch (o?.kind) {
    case "clock":
      switch (o.clock) {
        case "minute": return now.getMinutes();
        case "weekday": return now.getDay();
        case "day": return now.getDate();
        case "month": return now.getMonth() + 1;
        default: return now.getHours();
      }
    case "number":
      return Number(o.n ?? 0);
    case "text":
      return String(o.text ?? "");
    default:
      switch (o?.field) {
        case "text": return input ? plainText(input) : "";
        case "names": return filesOf(input).map((f) => f.filename).join(", ");
        default: return valueCount(input);
      }
  }
}

const looksNumeric = (v: string | number): boolean => {
  if (typeof v === "number") return true;
  const s = v.trim();
  return s !== "" && !Number.isNaN(Number(s));
};

/** Text comparisons ignore case throughout — nobody wiring up "contains
 *  MULT20015" means the capitals. */
const norm = (v: string | number): string => String(v).trim().toLowerCase();

function testRule(rule: Rule, input: Value | undefined, now: Date): boolean {
  const left = operandValue(rule.left, input, now);

  if (rule.op === "empty" || rule.op === "notEmpty") {
    const empty = left === 0 || norm(left) === "";
    return rule.op === "empty" ? empty : !empty;
  }

  const right = operandValue(rule.right, input, now);
  const numeric = looksNumeric(left) && looksNumeric(right);
  const [l, r] = [Number(left), Number(right)];

  switch (rule.op) {
    case "gt": return numeric && l > r;
    case "gte": return numeric && l >= r;
    case "lt": return numeric && l < r;
    case "lte": return numeric && l <= r;
    case "eq": return numeric ? l === r : norm(left) === norm(right);
    case "ne": return numeric ? l !== r : norm(left) !== norm(right);
    case "contains": return norm(left).includes(norm(right));
    case "notContains": return !norm(left).includes(norm(right));
    case "startsWith": return norm(left).startsWith(norm(right));
    case "endsWith": return norm(left).endsWith(norm(right));
    case "matches":
      // A half-typed pattern is a rule that does not match yet, not a run that
      // dies at 3am with a SyntaxError.
      try {
        return new RegExp(String(right), "i").test(String(left));
      } catch {
        return false;
      }
    default:
      return false;
  }
}

function branchPasses(b: Branch, input: Value | undefined, now: Date): boolean {
  if (b.rules.length === 0) return true;
  return b.match === "any"
    ? b.rules.some((r) => testRule(r, input, now))
    : b.rules.every((r) => testRule(r, input, now));
}

// ── Parsing and upgrades ─────────────────────────────────────────────────────

/** Old-style link: `[from, to, sourceHandle?]`, before ports existed. */
type LegacyLink = [string, string, string?];

const TEXT_FIELDS = ["prompt", "system", "title", "body"];

/** Placeholders that used to read from a hidden per-run fact bag. They now
 *  name the node's own input slots, so a graph written against the old names
 *  keeps saying what it meant. */
const LEGACY_VARS: Record<string, string> = {
  text: "{{input}}",
  fileList: "{{input}}",
  newFiles: "{{input.count}}",
  updatedFiles: "{{input.count}}",
  changedFiles: "{{input.count}}",
};

function upgradeText(v: unknown, ownSlots: Set<string>): unknown {
  if (typeof v !== "string") return v;
  return v.replace(/\{\{\s*(\w+)\s*\}\}/g, (m, key) =>
    // A node's own slot always wins. The retired names are ordinary words, and
    // an AI node's slots are whatever the user typed — someone who names a slot
    // `text` and writes `{{text}}` means their slot, and since this runs on
    // every parse the rewrite would otherwise eat it silently and for ever.
    ownSlots.has(key) ? m : (LEGACY_VARS[key] ?? m),
  );
}

/**
 * Bring a stored graph up to the current shape.
 *
 * Runs on every parse and must therefore be idempotent and deterministic —
 * the ids it invents are derived from the node it replaces, not from a
 * counter, so parsing twice gives the same graph. `migrateAutomationGraphs`
 * writes the result back once at startup; until then the app simply runs the
 * upgraded shape.
 */
function upgradeGraph(nodes: any[], links: any[]): AutomationGraph {
  const out: AutomationNode[] = [];
  const extra: AutomationLink[] = [];

  for (const raw of nodes) {
    if (!raw || typeof raw.id !== "string") continue;
    const node: AutomationNode = {
      id: raw.id,
      kind: raw.kind,
      config: { ...(raw.config ?? {}) },
      position: raw.position,
    };

    // The old all-in-one digest: summarise every changed file *and* write the
    // Inbox item. Split into the two nodes it always was underneath, so the
    // wire between them is visible and either half can be replaced.
    if ((node.kind as string) === "action.scrape_digest") {
      node.kind = "action.summarise";
      node.config = { instruction: "" };
      const inbox: AutomationNode = {
        id: `${node.id}_inbox`,
        kind: "action.inbox",
        config: { title: "Sync digest — {{input.count}} files", body: "{{input}}" },
        position: node.position
          ? { x: node.position.x + 300, y: node.position.y }
          : undefined,
      };
      out.push(node, inbox);
      extra.push({ from: node.id, fromPort: "summaries", to: inbox.id, toPort: "input" });
      continue;
    }

    // Conditions used to name a field from the hidden fact bag; the file
    // counts among them are now whatever is wired into the node.
    if (node.kind === "condition.if" && node.config.field != null) {
      const f = node.config.field;
      node.config.source = f === "hour" || f === "weekday" ? f : "input";
      delete node.config.field;
    }

    // A condition used to be one test with a `true`/`false` pair of ports; it
    // is now a router. The single test becomes the one branch, and its id is
    // pinned to "true" so wires already drawn off that port stay attached —
    // the old `false` port becomes `else`, rewritten on the links below.
    if (node.kind === "condition.if" && !Array.isArray(node.config.branches)) {
      const source = node.config.source ?? "input";
      const left: Operand =
        source === "input"
          ? { kind: "input", field: "count" }
          : { kind: "clock", clock: source };
      const n = Number(node.config.value ?? 0);
      node.config = {
        branches: [
          {
            id: "true",
            label: "Yes",
            match: "all",
            rules: [
              {
                left,
                op: (node.config.op ?? "gt") as RuleOp,
                right: { kind: "number", n: Number.isNaN(n) ? 0 : n },
              },
            ],
          },
        ],
        otherwise: true,
      };
    }

    const ownSlots = new Set(aiSlots(node.config).map((p) => p.id));
    for (const key of TEXT_FIELDS) {
      if (key in node.config) node.config[key] = upgradeText(node.config[key], ownSlots);
    }
    out.push(node);
  }

  const byId = new Map(out.map((n) => [n.id, n]));

  /** The condition's old "No" port is the router's catch-all. Idempotent
   *  because the port it renames no longer exists afterwards. */
  const outPort = (from: string, port: string): string =>
    port === "false" && byId.get(from)?.kind === "condition.if" ? "else" : port;

  const upgraded: AutomationLink[] = [];
  for (const raw of links) {
    if (Array.isArray(raw)) {
      const [from, to, handle] = raw as LegacyLink;
      const src = byId.get(from);
      const dst = byId.get(to);
      if (!src || !dst) continue;
      upgraded.push({
        from,
        // A condition's branch was the only handle the old shape had.
        fromPort: outPort(from, handle ?? defaultOutPort(src)),
        to,
        toPort: defaultInPort(dst),
      });
    } else if (raw && typeof raw.from === "string" && typeof raw.to === "string") {
      upgraded.push({
        from: raw.from,
        fromPort: outPort(raw.from, raw.fromPort ?? "then"),
        to: raw.to,
        toPort: raw.toPort ?? "input",
      });
    }
  }

  return { nodes: out, links: [...upgraded, ...extra] };
}

export function parseGraph(raw: string): AutomationGraph {
  try {
    const g = JSON.parse(raw);
    return upgradeGraph(
      Array.isArray(g.nodes) ? g.nodes : [],
      Array.isArray(g.links) ? g.links : [],
    );
  } catch {
    return EMPTY_GRAPH;
  }
}

export const serializeGraph = (g: AutomationGraph): string => JSON.stringify(g);

/** Persist the upgrade for every stored graph that needs one. Idempotent, and
 *  cheap enough to run at every launch: a graph already in the new shape
 *  serialises back to what is already in the row and is left alone. */
export async function migrateAutomationGraphs(): Promise<void> {
  for (const a of await getAutomations()) {
    const next = serializeGraph(parseGraph(a.graph));
    if (next !== a.graph) await updateAutomation(a.id, { graph: next });
  }
}

export function triggerNodes(g: AutomationGraph): AutomationNode[] {
  return g.nodes.filter((n) => isTrigger(n.kind));
}

export function triggerNode(g: AutomationGraph): AutomationNode | null {
  return triggerNodes(g)[0] ?? null;
}

/** Every node reachable from the triggers, in walk order — what actually runs.
 *  Nodes left unconnected on the canvas are excluded, which is the point. */
export function reachable(g: AutomationGraph): AutomationNode[] {
  const byId = new Map(g.nodes.map((n) => [n.id, n]));
  const seen = new Set<string>();
  const out: AutomationNode[] = [];
  const queue = triggerNodes(g).map((n) => n.id);
  while (queue.length) {
    const id = queue.shift()!;
    if (seen.has(id)) continue;
    seen.add(id);
    const node = byId.get(id);
    if (!node) continue;
    if (!isTrigger(node.kind)) out.push(node);
    for (const l of g.links) if (l.from === id) queue.push(l.to);
  }
  return out;
}

/** Wires arriving at a node, in draw order. */
export const incomingLinks = (g: AutomationGraph, nodeId: string): AutomationLink[] =>
  g.links.filter((l) => l.to === nodeId);

/** "On an event · New files" — what a slot is fed by, for the inspector. */
export function bindingLabel(g: AutomationGraph, link: AutomationLink): string {
  const src = g.nodes.find((n) => n.id === link.from);
  if (!src) return "—";
  const port = portById(outputPorts(src), link.fromPort);
  return `${NODE_TITLES[src.kind]} · ${port?.label ?? link.fromPort}`;
}

// ── Labels ───────────────────────────────────────────────────────────────────

const WEEKDAYS = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

export const EVENT_LABELS: Record<string, string> = {
  "sync-complete": "a sync finishes",
  "app-start": "Oculus starts",
};

/** Which aspect of the wired-in value a rule weighs — `text` is what the value
 *  says without running anything, so a summaries slot reads as empty here. */
export const INPUT_FIELDS: Record<NonNullable<Operand["field"]>, string> = {
  count: "How many",
  text: "Its text",
  names: "Their filenames",
};

export const CLOCK_FIELDS: Record<NonNullable<Operand["clock"]>, string> = {
  hour: "Hour of day",
  minute: "Minute of hour",
  weekday: "Day of week",
  day: "Day of month",
  month: "Month",
};

export const RULE_OPS: Record<RuleOp, string> = {
  gt: "is more than",
  gte: "is at least",
  lt: "is fewer than",
  lte: "is at most",
  eq: "is",
  ne: "is not",
  contains: "contains",
  notContains: "does not contain",
  startsWith: "starts with",
  endsWith: "ends with",
  matches: "matches",
  empty: "is empty",
  notEmpty: "is not empty",
};

/** What "Read Calendar" may look at. `note` is here because `action.calendar`
 *  can write one: a graph that could put a reminder on the calendar but never
 *  read it back would only be able to talk to itself. */
export const CALENDAR_KINDS: Record<string, string> = {
  class: "Classes",
  due: "Due dates",
  lecture: "Lectures",
  note: "Notes",
};

export const EVENT_KINDS: Record<string, string> = {
  due: "Due date",
  class: "Class",
  note: "Note",
};

export const INBOX_SCOPES: Record<string, string> = {
  unread: "Unread only",
  all: "Everything",
};

export const NODE_TITLES: Record<NodeKind, string> = {
  "trigger.schedule": "On a schedule",
  "trigger.event": "On an event",
  "condition.if": "Split the path",
  "source.inbox": "Read my Inbox",
  "source.calendar": "Read my calendar",
  "source.files": "Read my files",
  "action.sync": "Sync my subjects",
  "action.summarise": "Summarise each file",
  "action.ai": "Ask AI",
  "action.inbox": "Add to Inbox",
  "action.calendar": "Add to calendar",
  "action.notify": "Send a notification",
};

/** The configured detail under a node's title — "Daily at 09:00". */
export function nodeSummary(n: AutomationNode): string {
  const c = n.config ?? {};
  switch (n.kind) {
    case "trigger.schedule":
      if (c.scheduleKind === "interval") return `Every ${formatMinutes(c.intervalMinutes ?? 0)}`;
      if (c.scheduleKind === "weekly") {
        const days: number[] = Array.isArray(c.days) ? c.days : [];
        const when = days.length ? days.map((d) => WEEKDAYS[d]).join(", ") : "no days";
        return `${when} at ${c.timeOfDay ?? "—"}`;
      }
      return `Daily at ${c.timeOfDay ?? "—"}`;
    case "trigger.event":
      return `When ${EVENT_LABELS[c.event] ?? c.event ?? "…"}`;
    case "condition.if": {
      const count = branchesOf(n).length;
      const branches = `${count} branch${count === 1 ? "" : "es"}`;
      return c.otherwise === false ? branches : `${branches}, then otherwise`;
    }
    case "source.inbox":
      return `${INBOX_SCOPES[c.scope ?? "unread"] ?? "Unread only"}, last ${plural(c.days ?? 7, "day")}`;
    case "source.calendar": {
      const kinds: string[] = Array.isArray(c.kinds) && c.kinds.length ? c.kinds : ["due"];
      const names = kinds.map((k) => CALENDAR_KINDS[k] ?? k);
      const what = [names[0], ...names.slice(1).map((s) => s.toLowerCase())].join(" and ");
      return `${what}, next ${plural(c.days ?? 7, "day")}`;
    }
    case "source.files": {
      const what = c.category ? humanizeSlug(String(c.category)) : "Files";
      return `${what}, last ${plural(c.days ?? 7, "day")}`;
    }
    case "action.sync":
      return "Scrape the selected subjects";
    case "action.summarise":
      return c.instruction ? String(c.instruction) : "One summary per file";
    case "action.ai":
      return c.prompt ? String(c.prompt) : "No prompt yet";
    case "action.inbox":
      return c.title ? String(c.title) : "Untitled note";
    case "action.calendar": {
      const when =
        c.when === "template"
          ? (c.at ? `on ${c.at}` : "on a date you have not set")
          : `in ${plural(c.offsetDays ?? 1, "day")} at ${c.timeOfDay ?? "09:00"}`;
      return `${c.title ? String(c.title) : "Untitled event"} · ${when}`;
    }
    case "action.notify":
      return c.title ? String(c.title) : "Untitled notification";
  }
}

const plural = (n: number, word: string): string =>
  `${n} ${word}${Number(n) === 1 ? "" : "s"}`;

/** One-line "trigger → action" summary for the list row. */
export function describeGraph(g: AutomationGraph): string {
  const triggers = triggerNodes(g);
  const head = triggers.length
    ? triggers.map(nodeSummary).join(" · ")
    : "No trigger";
  const steps = reachable(g);
  if (steps.length === 0) return `${head} → nothing yet`;
  const tail =
    steps.length <= 3
      ? steps.map((n) => NODE_TITLES[n.kind].toLowerCase()).join(" → ")
      : `${steps.length} steps`;
  return `${head} → ${tail}`;
}

/** An interval is one stored number of minutes; the unit is only ever how it
 *  is read back, so the largest unit it divides evenly into wins. */
export function formatMinutes(m: number): string {
  if (m < 60) return `${m} min`;
  if (m % 10080 === 0) return plural(m / 10080, "week");
  if (m % 1440 === 0) return plural(m / 1440, "day");
  if (m % 60 === 0) return plural(m / 60, "hour");
  return `${m} min`;
}

// ── Schedule timing ──────────────────────────────────────────────────────────
//
// A trigger's anchor marks the period already covered — creation, its last
// firing, or the moment the automation was re-enabled — so a schedule is due
// when a fire-point has passed that its anchor predates. Anchors are per
// trigger: "daily at 09:00" must still come due on a graph whose other
// trigger runs every 30 minutes.

interface TriggerState {
  anchor: number;
  fired?: number;
}

export function parseTriggerState(raw: string): Record<string, TriggerState> {
  try {
    const v = JSON.parse(raw || "{}");
    return v && typeof v === "object" ? v : {};
  } catch {
    return {};
  }
}

/** A trigger's anchor, falling back to the row's — which is what migrated
 *  graphs and never-fired triggers use. */
function anchorOf(a: DbAutomation, nodeId: string): number | null {
  const own = parseTriggerState(a.trigger_state)[nodeId]?.anchor;
  return own ?? sqliteUtcToMs(a.anchor_at);
}

/** Most recent fire-point at or before `now`, or null if there is none. */
function lastDuePointMs(node: AutomationNode, anchor: number, now: Date): number | null {
  const { scheduleKind, timeOfDay, intervalMinutes, days } = node.config ?? {};

  if (scheduleKind === "interval") {
    // Intervals are anchored, not wall-clock aligned.
    if (!intervalMinutes) return null;
    const point = anchor + intervalMinutes * 60_000;
    return point <= now.getTime() ? point : null;
  }

  if (!timeOfDay) return null;
  const [h, m] = String(timeOfDay).split(":").map(Number);
  if (Number.isNaN(h) || Number.isNaN(m)) return null;

  if (scheduleKind === "weekly") {
    const wanted: number[] = Array.isArray(days) ? days : [];
    if (wanted.length === 0) return null;
    // Walk back a week at most; the first matching day whose time has passed
    // is the point this schedule most recently owed.
    for (let back = 0; back < 8; back++) {
      const d = new Date(now.getFullYear(), now.getMonth(), now.getDate() - back, h, m);
      if (d.getTime() <= now.getTime() && wanted.includes(d.getDay())) return d.getTime();
    }
    return null;
  }

  const point = new Date(now.getFullYear(), now.getMonth(), now.getDate(), h, m);
  if (point.getTime() > now.getTime()) point.setDate(point.getDate() - 1);
  return point.getTime();
}

/** Schedule triggers of an enabled automation that owe a run right now. */
export function dueTriggers(a: DbAutomation, now = new Date()): AutomationNode[] {
  if (!a.enabled) return [];
  return triggerNodes(parseGraph(a.graph)).filter((t) => {
    if (t.kind !== "trigger.schedule") return false;
    const anchor = anchorOf(a, t.id);
    if (anchor == null) return false;
    const point = lastDuePointMs(t, anchor, now);
    return point != null && anchor < point;
  });
}

export const isAutomationDue = (a: DbAutomation, now = new Date()): boolean =>
  dueTriggers(a, now).length > 0;

/** Next firing of one schedule trigger in epoch ms. Overdue reads as "now". */
function nextFireOf(a: DbAutomation, t: AutomationNode, now: Date): number | null {
  const anchor = anchorOf(a, t.id);
  if (anchor == null) return null;
  const point = lastDuePointMs(t, anchor, now);
  if (point != null && anchor < point) return now.getTime();

  const { scheduleKind, timeOfDay, intervalMinutes, days } = t.config ?? {};
  if (scheduleKind === "interval") {
    return intervalMinutes ? anchor + intervalMinutes * 60_000 : null;
  }
  if (!timeOfDay) return null;
  const [h, m] = String(timeOfDay).split(":").map(Number);
  if (Number.isNaN(h) || Number.isNaN(m)) return null;

  if (scheduleKind === "weekly") {
    const wanted: number[] = Array.isArray(days) ? days : [];
    if (wanted.length === 0) return null;
    for (let ahead = 0; ahead < 8; ahead++) {
      const d = new Date(now.getFullYear(), now.getMonth(), now.getDate() + ahead, h, m);
      if (d.getTime() > now.getTime() && wanted.includes(d.getDay())) return d.getTime();
    }
    return null;
  }
  const d = new Date(now.getFullYear(), now.getMonth(), now.getDate(), h, m);
  if (d.getTime() <= now.getTime()) d.setDate(d.getDate() + 1);
  return d.getTime();
}

/** Earliest next firing across every schedule trigger, for the list row. */
export function nextFireMs(a: DbAutomation, now = new Date()): number | null {
  const points = triggerNodes(parseGraph(a.graph))
    .filter((t) => t.kind === "trigger.schedule")
    .map((t) => nextFireOf(a, t, now))
    .filter((v): v is number => v != null);
  return points.length ? Math.min(...points) : null;
}

// ── Values ───────────────────────────────────────────────────────────────────

export interface FileRef {
  subjectId: number | null;
  subjectCode: string | null;
  relativePath: string;
  filename: string;
  action: string;
}

/**
 * What travels along a wire.
 *
 * `summaries` is a promise of work rather than the work: a file list plus the
 * instruction to summarise each one with. The node that consumes it does the
 * model calls, which is what lets "Add to Inbox" put the item on screen with
 * one pending row per file and fill them in as they finish — and lets a quit
 * mid-way resume from those rows. A summariser that did the work eagerly
 * could only hand over a finished blob, minutes later, with nothing to show
 * in between and nothing to resume from.
 */
export type Value =
  | { kind: "signal" }
  | { kind: "files"; files: FileRef[]; runId?: number }
  | { kind: "summaries"; files: FileRef[]; instruction: string; runId?: number }
  /** `n` is how many things the text describes, when the node that made it
   *  knew — a calendar read hands on both its listing and its length. Without
   *  it a condition could only ever gate on the count *instead of* the
   *  content: the count leaves by its own port, and a branch forwards the one
   *  value it was given. */
  | { kind: "text"; text: string; n?: number }
  | { kind: "number"; n: number };

const SIGNAL: Value = { kind: "signal" };

export const filesOf = (v: Value | undefined): FileRef[] =>
  v && (v.kind === "files" || v.kind === "summaries") ? v.files : [];

export const runIdOf = (v: Value | undefined): number | undefined =>
  v && (v.kind === "files" || v.kind === "summaries") ? v.runId : undefined;

/** How many things a value carries — what a condition weighs. */
export function valueCount(v: Value | undefined): number {
  if (!v) return 0;
  switch (v.kind) {
    case "files":
    case "summaries": return v.files.length;
    case "number": return v.n;
    case "text": return v.n ?? (v.text.trim() ? 1 : 0);
    case "signal": return 0;
  }
}

/** Several wires into one slot read as one value: file lists concatenate,
 *  text runs together. A slot fed both new and updated files sees both. */
function mergeValues(values: Value[]): Value {
  if (values.length === 0) return SIGNAL;
  if (values.length === 1) return values[0];
  const summaries = values.find((v) => v.kind === "summaries");
  if (summaries) {
    const files = values.flatMap(filesOf);
    return { kind: "summaries", files: dedupe(files), instruction: (summaries as any).instruction, runId: values.map(runIdOf).find((r) => r != null) };
  }
  if (values.every((v) => v.kind === "files")) {
    return { kind: "files", files: dedupe(values.flatMap(filesOf)), runId: values.map(runIdOf).find((r) => r != null) };
  }
  // Mixed kinds have no common shape, so they meet as text — a file list and
  // a generated paragraph in one slot read as both, not as whichever was
  // drawn last.
  const texts = values.map(plainText).filter(Boolean);
  if (!texts.length) return values[0];
  // Two counted listings in one slot describe both sets, so their lengths add.
  const counted = values.filter((v) => v.kind === "text" && v.n != null);
  const n = counted.length ? counted.reduce((sum, v) => sum + ((v as any).n ?? 0), 0) : undefined;
  return { kind: "text", text: texts.join("\n\n"), ...(n != null ? { n } : {}) };
}

const dedupe = (files: FileRef[]): FileRef[] => {
  const seen = new Set<string>();
  return files.filter((f) => !seen.has(f.relativePath) && seen.add(f.relativePath));
};

const fileLine = (f: FileRef) => `- ${f.subjectCode ?? "?"}: ${f.filename}`;

/** Everything a value says without running anything — a summaries value has
 *  nothing to say until its model calls happen, so it says nothing here. */
function plainText(v: Value): string {
  switch (v.kind) {
    case "text": return v.text;
    case "number": return String(v.n);
    case "files": return v.files.map(fileLine).join("\n");
    default: return "";
  }
}

/** A value as prompt-ready text. Reading a `summaries` value is what makes it
 *  happen, so a prompt that mentions the slot pays for the summaries and a
 *  prompt that does not, does not. */
async function valueToText(v: Value | undefined): Promise<string> {
  if (!v) return "";
  switch (v.kind) {
    case "signal":
    case "text":
    case "number":
    case "files": return plainText(v);
    case "summaries": {
      const done = await materialiseSummaries(v.files, v.instruction);
      return done.map((s) => `### ${s.file.filename}\n${s.text}`).join("\n\n");
    }
  }
}

// ── Templates ────────────────────────────────────────────────────────────────

/** Names every text field understands on top of the node's own input slots. */
export const GLOBAL_VARS = ["name", "date", "time"];

/**
 * `{{slot}}` substitution over a node's inputs.
 *
 * The names are the node's own input slots — `{{input}}` is whatever you wired
 * into the slot called "input" — plus the three globals. `{{slot.count}}` is
 * how many things came in and `{{slot.names}}` is their filenames, so a prompt
 * can say "3 new files" without the list.
 */
async function render(
  template: string,
  inputs: Record<string, Value>,
  state: RunState,
): Promise<string> {
  const now = new Date();
  const globals: Record<string, string> = {
    name: state.name,
    date: now.toLocaleDateString(),
    time: now.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }),
  };

  const out: string[] = [];
  const src = String(template ?? "");
  const re = /\{\{\s*(\w+)(?:\.(\w+))?\s*\}\}/g;
  let last = 0;
  for (let m = re.exec(src); m; m = re.exec(src)) {
    out.push(src.slice(last, m.index));
    last = m.index + m[0].length;
    const [, name, field] = m;
    if (name in inputs) {
      const v = inputs[name];
      out.push(
        field === "count"
          ? String(valueCount(v))
          : field === "names"
            ? filesOf(v).map((f) => f.filename).join(", ")
            : await valueToText(v),
      );
    } else if (!field && name in globals) {
      out.push(globals[name]);
    } else {
      out.push(m[0]);
    }
  }
  out.push(src.slice(last));
  return out.join("");
}

// ── Execution ────────────────────────────────────────────────────────────────

/** What the caller knows that the graph cannot work out for itself. */
export interface RunContext {
  /** The sync run a `sync-complete` trigger is firing for. */
  runId?: number;
}

interface RunState {
  name: string;
  runId?: number;
}

/** The file lists a sync run produced, as the trigger's three output ports. */
async function runOutputs(runId: number | undefined): Promise<Record<string, Value>> {
  const rows = runId == null ? [] : await getSyncRunFiles(runId).catch(() => []);
  const files: FileRef[] = rows
    .filter((f) => f.action === "new" || f.action === "updated")
    .map((f) => ({
      subjectId: f.subject_id,
      subjectCode: f.subject_code,
      relativePath: f.relative_path,
      filename: f.relative_path.split("/").pop() ?? f.relative_path,
      action: f.action,
    }));
  const pick = (action: string) => files.filter((f) => f.action === action);
  return {
    new: { kind: "files", files: pick("new"), runId },
    updated: { kind: "files", files: pick("updated"), runId },
    changed: { kind: "files", files, runId },
  };
}

/** What a trigger hands out the moment it fires. A `sync-complete` trigger
 *  run by hand from the editor has no run of its own, so it describes the
 *  latest one — otherwise "Run now" could only ever test an empty graph. */
async function triggerOutputs(
  node: AutomationNode,
  ctx: RunContext,
): Promise<Record<string, Value>> {
  if (node.kind === "trigger.event" && node.config?.event === "sync-complete") {
    const runId = ctx.runId ?? (await getLatestSyncRunId().catch(() => null)) ?? undefined;
    return runOutputs(runId);
  }
  return { then: SIGNAL };
}

/** OS notification. Permission is asked for on first use, not at launch —
 *  a prompt only earns its place once an automation actually wants one. */
async function notify(title: string, body: string): Promise<void> {
  let granted = await isPermissionGranted();
  if (!granted) granted = (await requestPermission()) === "granted";
  if (granted) sendNotification({ title, body });
}

/**
 * The calendar's three layers as one list, inside a forward window.
 *
 * Lectures are only fetched when asked for, and they are here at all because
 * most UniMelb subjects publish nothing to the Canvas calendar — the Echo360
 * recordings *are* the timetable (see `docs/calendar.md`), so a node reading
 * "classes this week" from `calendar_events` alone would come back empty.
 * Locally added events merge in the same way: an event this automation wrote
 * last week is as real as one Canvas sent.
 */
async function calendarWindow(
  kinds: string[],
  days: number,
  subjectIds: number[],
): Promise<Array<{ kind: string; title: string; start: Date; subjectCode: string | null }>> {
  const now = Date.now();
  const until = now + Math.max(0, days) * 86_400_000;
  const rows: Array<{
    kind: string;
    title: string;
    start: Date;
    subjectCode: string | null;
    subjectId: number | null;
  }> = [];

  if (kinds.includes("class") || kinds.includes("due")) {
    for (const e of await getCalendarEvents()) {
      rows.push({
        kind: e.kind,
        title: e.title,
        start: new Date(e.start_at),
        subjectCode: e.subject_code,
        subjectId: e.subject_id,
      });
    }
  }
  for (const e of await getLocalEvents()) {
    rows.push({
      kind: e.kind,
      title: e.title,
      start: new Date(e.start_at),
      subjectCode: e.subject_code,
      subjectId: e.subject_id,
    });
  }
  if (kinds.includes("lecture")) {
    for (const l of await getAllLectures()) {
      rows.push({
        kind: "lecture",
        title: l.title,
        start: new Date(l.date),
        subjectCode: l.subject_code,
        subjectId: l.subject_id,
      });
    }
  }

  return rows
    .filter((r) => kinds.includes(r.kind))
    .filter((r) => subjectIds.length === 0 || (r.subjectId != null && subjectIds.includes(r.subjectId)))
    .filter((r) => {
      const t = r.start.getTime();
      return !Number.isNaN(t) && t >= now && t <= until;
    })
    .sort((a, b) => a.start.getTime() - b.start.getTime());
}

const eventWhen = (d: Date): string =>
  d.toLocaleString([], {
    weekday: "short",
    day: "numeric",
    month: "short",
    hour: "2-digit",
    minute: "2-digit",
  });

/** A bare `YYYY-MM-DD` parses as UTC midnight, which lands on the wrong side
 *  of the day in Melbourne — so a date-only template is read on the local
 *  calendar and given the node's time of day. */
const DATE_ONLY = /^\d{4}-\d{2}-\d{2}$/;

function timeOfDayParts(v: unknown): [number, number] {
  const [h, m] = String(v ?? "09:00").split(":").map(Number);
  return [Number.isFinite(h) ? h : 9, Number.isFinite(m) ? m : 0];
}

/**
 * When the event the node is about to write starts.
 *
 * A template that renders to something no `Date` can read throws rather than
 * writing a row at the epoch: the walk is fail-fast on purpose, and a calendar
 * quietly full of 1970 is worse than a run that stopped and said why.
 */
async function eventStartAt(
  c: Record<string, any>,
  inputs: Record<string, Value>,
  state: RunState,
): Promise<Date> {
  const [h, m] = timeOfDayParts(c.timeOfDay);

  if (c.when === "template") {
    const raw = (await render(c.at ?? "", inputs, state)).trim();
    if (DATE_ONLY.test(raw)) {
      const [y, mo, d] = raw.split("-").map(Number);
      return new Date(y, mo - 1, d, h, m);
    }
    const at = new Date(raw);
    if (!raw || Number.isNaN(at.getTime())) {
      throw new Error(`Add to calendar: "${raw}" is not a date or time`);
    }
    return at;
  }

  const offset = Number(c.offsetDays ?? 1);
  const d = new Date();
  d.setDate(d.getDate() + (Number.isFinite(offset) ? offset : 1));
  d.setHours(h, m, 0, 0);
  return d;
}

/**
 * Run one node against its resolved inputs and return what it puts on each of
 * its output ports. A port left out of the returned record is dead: nothing
 * downstream of it runs, which is how a condition prunes the walk.
 */
async function runNode(
  node: AutomationNode,
  inputs: Record<string, Value>,
  state: RunState,
): Promise<Record<string, Value>> {
  const c = node.config ?? {};

  switch (node.kind) {
    case "condition.if": {
      // First match wins and nothing else fires: this routes, it does not fan
      // out. Every branch carries the incoming value through, so a condition
      // can sit in the middle of a wire without breaking it — and a port left
      // out of this record is dead, which is how the untaken branches prune
      // the rest of the walk.
      const now = new Date();
      const value = inputs.input ?? SIGNAL;
      for (const b of branchesOf(node)) {
        if (branchPasses(b, inputs.input, now)) return { [b.id]: value };
      }
      return c.otherwise === false ? {} : { else: value };
    }

    case "source.inbox": {
      const items = await getInboxDigest({
        scope: c.scope === "all" ? "all" : "unread",
        days: Number(c.days ?? 7),
        limit: Number(c.limit ?? 20),
      });
      // One block of markdown rather than a list value: what a prompt wants
      // from the Inbox is what the items *said*, not references to them.
      const text = items
        .map((i) => `## ${i.title}\n\n${i.body}`.trim())
        .join("\n\n");
      return {
        items: { kind: "text", text, n: items.length },
        count: { kind: "number", n: items.length },
      };
    }

    case "source.calendar": {
      const kinds: string[] = Array.isArray(c.kinds) && c.kinds.length ? c.kinds : ["due"];
      const events = await calendarWindow(
        kinds,
        Number(c.days ?? 7),
        Array.isArray(c.subjectIds) ? c.subjectIds : [],
      );
      const text = events
        .map((e) => `- ${eventWhen(e.start)} · ${e.subjectCode ?? "—"}: ${e.title}`)
        .join("\n");
      return {
        events: { kind: "text", text, n: events.length },
        count: { kind: "number", n: events.length },
      };
    }

    case "source.files": {
      const rows = await getFilesForAutomation({
        days: Number(c.days ?? 7),
        subjectIds: Array.isArray(c.subjectIds) ? c.subjectIds : [],
        category: c.category ?? null,
        limit: Number(c.limit ?? 50),
      });
      // A `files` value, not text, so this port feeds "Summarise each file"
      // exactly as a sync's file lists do.
      const files: FileRef[] = rows.map((f) => ({
        subjectId: f.subject_id,
        subjectCode: f.subject_code,
        relativePath: f.relative_path,
        filename: f.filename,
        action: "existing",
      }));
      return {
        files: { kind: "files", files },
        count: { kind: "number", n: files.length },
      };
    }

    case "action.sync": {
      const runId = await triggerSync("scheduled");
      return runOutputs(runId);
    }

    case "action.summarise": {
      const files = filesOf(inputs.files);
      return {
        summaries: {
          kind: "summaries",
          files,
          instruction: String(c.instruction ?? ""),
          runId: runIdOf(inputs.files) ?? state.runId,
        },
      };
    }

    case "action.ai": {
      const text = await invoke<string>("llm_generate", {
        prompt: await render(c.prompt ?? "", inputs, state),
        system: c.system ? await render(c.system, inputs, state) : null,
      });
      return { text: { kind: "text", text } };
    }

    case "action.inbox": {
      const title = await render(c.title || "Automation note", inputs, state);
      const value = inputs.input;
      if (value?.kind === "summaries" && value.files.length > 0) {
        // One item, one row per file, filled in as the summaries land.
        await deliverSummaries(title, value.files, value.instruction, value.runId ?? state.runId ?? null);
      } else {
        await addInboxNote(
          title,
          await render(c.body ?? "{{input}}", inputs, state),
          runIdOf(value) ?? state.runId ?? null,
        );
      }
      await useInboxStore.getState().refresh();
      return { then: SIGNAL };
    }

    case "action.calendar": {
      const notes = (await render(c.notes ?? "{{input}}", inputs, state)).trim();
      await addLocalEvent({
        subjectId: c.subjectId ?? null,
        kind: EVENT_KINDS[c.kind] ? c.kind : "note",
        title: (await render(c.title || "{{name}}", inputs, state)).trim() || state.name,
        startAt: (await eventStartAt(c, inputs, state)).toISOString(),
        allDay: false,
        notes: notes || null,
      });
      return { then: SIGNAL };
    }

    case "action.notify": {
      await notify(
        await render(c.title || state.name, inputs, state),
        await render(c.body ?? "{{input}}", inputs, state),
      );
      return { then: SIGNAL };
    }

    default:
      return { then: SIGNAL };
  }
}

/**
 * Walk the graph from one trigger.
 *
 * A node runs once every wire into it has settled — its source has either run
 * or been skipped — and only if at least one of those wires actually carries a
 * value. That is what makes fan-in work: a node fed by two branches sees both,
 * rather than running early on whichever arrived first. A node reached only
 * through a condition's untaken branch is skipped, and the skip propagates.
 *
 * Sequential and fail-fast: a node that throws stops the run, because an
 * Inbox item about a sync that never happened is nonsense. A cycle drawn on
 * the canvas never settles and so simply never runs.
 */
export async function runFrom(
  a: DbAutomation,
  startId: string,
  ctx: RunContext = {},
): Promise<void> {
  const g = parseGraph(a.graph);
  const byId = new Map(g.nodes.map((n) => [n.id, n]));
  const start = byId.get(startId);
  if (!start) return;

  const state: RunState = { name: a.name, runId: ctx.runId };
  const outputs = new Map<string, Record<string, Value>>();
  const settled = new Set<string>([startId]);
  outputs.set(startId, await triggerOutputs(start, ctx));

  for (;;) {
    const ready = g.nodes.filter((n) => {
      if (settled.has(n.id)) return false;
      const wires = incomingLinks(g, n.id);
      return wires.length > 0 && wires.every((l) => settled.has(l.from));
    });
    if (ready.length === 0) break;

    for (const node of ready) {
      settled.add(node.id);
      const live = incomingLinks(g, node.id).filter(
        (l) => outputs.get(l.from)?.[l.fromPort] !== undefined,
      );
      if (live.length === 0) continue; // only dead branches reach here

      const bySlot = new Map<string, Value[]>();
      for (const l of live) {
        const v = outputs.get(l.from)![l.fromPort];
        bySlot.set(l.toPort, [...(bySlot.get(l.toPort) ?? []), v]);
      }
      const inputs: Record<string, Value> = {};
      for (const [slot, values] of bySlot) inputs[slot] = mergeValues(values);

      outputs.set(node.id, await runNode(node, inputs, state));
    }
  }
}

/** Stamp the firing before the work, so a failing action cannot refire every
 *  tick, then run the graph from that trigger. */
export async function fireTrigger(
  a: DbAutomation,
  trigger: AutomationNode,
  ctx: RunContext = {},
): Promise<void> {
  const now = Date.now();
  const state = { ...parseTriggerState(a.trigger_state), [trigger.id]: { anchor: now, fired: now } };
  await setAutomationTriggerState(a.id, JSON.stringify(state));
  await markAutomationFired(a.id);
  await runFrom(a, trigger.id, ctx);
}

/** Run every enabled graph whose trigger listens for `event`. Called from
 *  wherever the event actually lands — `useBackendEvents` for sync-complete. */
export async function runEventAutomations(event: string, ctx: RunContext): Promise<void> {
  for (const a of await getAutomations()) {
    if (!a.enabled) continue;
    for (const t of triggerNodes(parseGraph(a.graph))) {
      if (t.kind === "trigger.event" && t.config?.event === event) {
        await fireTrigger(a, t, ctx);
      }
    }
  }
}

/** Run a graph on demand from the editor's "Run now" button — every trigger's
 *  path, without touching the schedule anchors. */
export async function runAutomationNow(a: DbAutomation, ctx: RunContext = {}): Promise<void> {
  for (const t of triggerNodes(parseGraph(a.graph))) {
    await runFrom(a, t.id, ctx);
  }
}

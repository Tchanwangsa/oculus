import { mayHoldWidget } from "@/components/markdown/openui/remarkWidgets";
import { sqliteUtcToMs } from "@/lib/format/format";
import {
  describeOculusCli,
  elapsedText,
  runEnded,
  runSwitch,
  steerMeta,
  parseToolMeta,
  thoughtLabel,
  thoughtMs,
  type CliIcon,
  type HarnessItem,
  type HarnessRun,
  type ToolKind,
} from "@/lib/harness";

/** How an answer row draws: the run's settled answer, the latest reply of a
 *  run still working (no Copy or Retry yet), or what a stopped or failed run
 *  kept. */
export type ReplyRole = "answer" | "provisional" | "partial";

/** A run's state as its fold draws it. A run in neither state (unsettled,
 *  with nothing running) is drawn as steps, never folded. */
export type RunState = "running" | "completed" | "interrupted" | "failed";

/** One run's work above its answer: earlier replies, reasoning and tools.
 *  A run the student steered has one per stretch between its steers. */
export interface WorkFold {
  kind: "work";
  id: string;
  thread: number;
  run: number;
  items: HarnessItem[];
  /** For a stretch a steer ended (`cut`), `completed`: it is not the run's
   *  live work, whatever the run is doing. */
  state: RunState;
  startedMs: number | null;
  endedMs: number | null;
  /** The stretch ended at a steer (`steer`), or was the attempt a restart
   *  steer superseded (`restart`: partial work, never the answer). */
  cut?: "steer" | "restart";
}

/** A run that changed agent, model or level from the run before it, or
 *  carried a context handoff: its question draws a divider above it. */
export interface RunChange {
  before?: HarnessRun;
  run: HarnessRun;
}

export type ViewRow =
  /** `asked`: for a reply, the student's question it answers (Retry).
   *  `key`: a run's reply is keyed by its place in the run (`replyKey`).
   *  `change`: on a run's question, what it was sent with that differs. */
  | { kind: "item"; item: HarnessItem; dim: boolean; role?: ReplyRole; asked?: HarnessItem; key?: string; change?: RunChange }
  | { kind: "bundle"; id: string; items: HarnessItem[] }
  | WorkFold
  /** The widget fences of a folded reply, drawn once outside its fold. */
  | { kind: "artifacts"; id: string; item: HarnessItem };

/** A reply carrying an interactive fence stays visible in a runless stretch:
 *  only a run's fold can pin the fence alone. */
const ARTIFACT = /```(openui|widget)\b/;

/** An earlier reply of a settled run (Rust's `RunSettled`): work, not the answer. */
export const isCommentary = (i: HarnessItem) =>
  i.kind === "assistant" && i.display_role === "commentary" && !ARTIFACT.test(i.content ?? "");

const isWork = (i: HarnessItem) => i.kind === "tool" || i.kind === "thinking" || isCommentary(i);

/** Rows with no run (unprompted turns, history from before runs): steps of
 *  tools and reasoning between messages, a finished one of two or more
 *  folded into a bundle. Nothing is regrouped by guesswork. A reply answers
 *  the question above it, and none after a turn the agent started itself. */
function stepRows(items: HarnessItem[], running: boolean, out: ViewRow[]): void {
  let step: HarnessItem[] = [];
  let question: HarnessItem | undefined;
  const close = (dim: boolean) => {
    // A lone earlier reply still folds, or it would read as a second answer.
    const fold = step.length >= 2 || step.some(isCommentary);
    if (fold && dim) out.push({ kind: "bundle", id: `b-${step[0].id}`, items: step });
    else for (const item of step) out.push({ kind: "item", item, dim });
    step = [];
  };
  for (const item of items) {
    if (isWork(item)) step.push(item);
    else {
      close(true);
      if (item.kind === "user") question = item;
      else if (item.kind === "unprompted" || item.kind === "continuation") question = undefined;
      const asked = item.kind === "assistant" ? question : undefined;
      out.push({ kind: "item", item, dim: false, ...(asked && { asked }) });
    }
  }
  // The trailing step is live while the turn runs.
  close(!running);
}

export interface RowContext {
  /** The thread's runs (`harnessStore.runs`). */
  runs?: Record<number, HarnessRun>;
  /** Text is streaming: it, not the latest committed reply, is the run's
   *  provisional answer. */
  streaming?: boolean;
}

const isFoldable = (i: HarnessItem) => i.kind === "tool" || i.kind === "thinking" || i.kind === "assistant";
const atMs = (i: HarnessItem) => sqliteUtcToMs(i.created_at) ?? null;

/** One run's rows (docs/harness.md, "A run's work folds above its answer").
 *  Each steer the student wrote into it ends a stretch: the work before it
 *  folds, closed, then the steer row, then what came after under a fold of
 *  its own. The run's one answer stays the last thing it drew. */
function runRows(
  segment: HarnessItem[],
  run: HarnessRun | undefined,
  before: HarnessRun | undefined,
  live: boolean,
  streaming: boolean,
  out: ViewRow[],
): void {
  const state: RunState | null = runEnded(run) ? (run!.status as RunState) : live ? "running" : null;
  // An unsettled run nobody is running (a load that missed its runs) is not guessed at.
  if (!state) return stepRows(segment, false, out);
  // A continuation heads its run like a question, but nothing re-asks it.
  const head = segment[0].kind === "user" || segment[0].kind === "continuation" ? segment[0] : null;
  const rest = head ? segment.slice(1) : segment;
  const stretches = splitAtAttempts(rest, run?.active_attempt_id);
  const tail = stretches[stretches.length - 1];
  let answer: HarnessItem | undefined;
  if (state === "running") {
    // Replies before the latest steer answered something the student has
    // since corrected: they are work, never the provisional answer.
    answer = streaming ? undefined : [...tail.items].reverse().find((i) => i.kind === "assistant");
  } else {
    answer = rest.find((i) => i.id === run!.answer_item_id) ?? rest.find((i) => i.display_role === "answer");
  }
  const change = run && (run.handoff || runSwitch(before, run)) ? { before, run } : undefined;
  if (head) out.push({ kind: "item", item: head, dim: false, ...(change && { change }) });
  const keys = new Map<HarnessItem, string>();
  let replies = 0;
  for (const item of rest) if (item.kind === "assistant") keys.set(item, replyKey(item.run_id!, replies++));
  const role: ReplyRole = state === "running" ? "provisional" : state === "completed" ? "answer" : "partial";
  const runId = run?.id ?? segment[0].run_id!;
  stretches.forEach((stretch, n) => {
    if (stretch.steer) out.push({ kind: "item", item: stretch.steer, dim: false });
    const work = stretch.items.filter((i) => i !== answer && isFoldable(i));
    const folded = new Set(work);
    const next = stretches[n + 1]?.steer;
    if (work.length) {
      const startedMs = stretch.steer ? atMs(stretch.steer) : (run?.started_ms ?? null);
      const cut = stretch.superseded ? "restart" : next ? (steerMeta(next).delivery === "restart" ? "restart" : "steer") : undefined;
      out.push({
        kind: "work",
        // The first stretch keeps the run's own id, so a run steered later
        // keeps its fold and the student's choice for it.
        id: stretch.steer ? `w-${runId}-${stretch.steer.id}` : `w-${runId}`,
        thread: segment[0].thread_id,
        run: runId,
        items: work,
        state: cut ? "completed" : state,
        startedMs,
        endedMs: next ? atMs(next) : state === "running" ? null : (run?.ended_ms ?? null),
        ...(cut && { cut }),
      });
      for (const item of work) {
        if (item.kind === "assistant" && mayHoldWidget(item.content)) out.push({ kind: "artifacts", id: `a-${item.id}`, item });
      }
    }
    for (const item of stretch.items) {
      const key = keys.get(item);
      if (item === answer) out.push({ kind: "item", item, dim: false, role, ...(head?.kind === "user" && { asked: head }), ...(key && { key }) });
      else if (!folded.has(item)) out.push({ kind: "item", item, dim: false });
    }
  });
}

interface Stretch {
  steer?: HarnessItem;
  items: HarnessItem[];
  superseded?: boolean;
}

/** Native steers cut at their row. A superseded attempt's output belongs
 *  before its corrections, even when cancellation saved its partial reply
 *  after them. Null ownership keeps its position; it is never inferred. */
function splitAtAttempts(rest: HarnessItem[], active: number | null | undefined): Stretch[] {
  const out: Stretch[] = [{ items: [] }];
  const ended = new Map<number, Stretch>();
  const positions = new Map<HarnessItem, Stretch>();
  for (const item of rest) {
    if (item.kind === "steer") {
      if (item.attempt_id != null && active != null && item.attempt_id !== active && !ended.has(item.attempt_id)) {
        const stretch = out[out.length - 1];
        stretch.superseded = true;
        ended.set(item.attempt_id, stretch);
      }
      out.push({ steer: item, items: [] });
    } else positions.set(item, out[out.length - 1]);
  }
  // Move all rows of an ended attempt together, preserving their stored
  // order inside its fold. Steers stay between attempts, in send order.
  for (const [item, position] of positions) {
    const target = item.attempt_id == null ? position : (ended.get(item.attempt_id) ?? position);
    target.items.push(item);
  }
  return out;
}

/** A run's reply's row key: its run and its place among the run's replies.
 *  The reply still streaming takes the key it will have once committed
 *  (`streamingReply`), so the row, and a widget's frame in it, carries over. */
export const replyKey = (run: number, ordinal: number) => `reply:${run}:${ordinal}`;

/** The run whose reply is streaming, and that reply's place in it: the
 *  thread's last row is the running run's (its question is written at
 *  send). Null for a turn with no run (one the agent started itself). */
export function streamingReply(items: HarnessItem[]): { run: number; ordinal: number } | null {
  const run = items[items.length - 1]?.run_id;
  if (run == null) return null;
  let ordinal = 0;
  for (let i = items.length - 1; i >= 0 && items[i].run_id === run; i--) if (items[i].kind === "assistant") ordinal++;
  return { run, ordinal };
}

/**
 * The thread as rows. Each run's contiguous rows become: its question (with
 * a divider above it where the run switched or carried a handoff), one
 * fold of its work, the widget fences of folded replies, then its answer and
 * outcome rows (notice, stop, error, refusal, compaction) in order. Rows with
 * no run keep the step bundles. `running` is the thread's: only its last run
 * can be in flight.
 */
export function buildRows(items: HarnessItem[], running: boolean, ctx: RowContext = {}): ViewRow[] {
  const out: ViewRow[] = [];
  const runs = ctx.runs ?? {};
  let before: HarnessRun | undefined;
  let i = 0;
  while (i < items.length) {
    const runId = items[i].run_id;
    let j = i + 1;
    if (runId == null) {
      while (j < items.length && items[j].run_id == null) j++;
      stepRows(items.slice(i, j), running && j === items.length, out);
    } else {
      while (j < items.length && items[j].run_id === runId) j++;
      const run = runs[runId];
      runRows(items.slice(i, j), run, before, running && j === items.length, !!ctx.streaming, out);
      before = runs[runId] ?? before;
    }
    i = j;
  }
  return out;
}

/** Inside an open fold: earlier replies as they are, and two or more tool or
 *  reasoning rows in a row as one bundle, except the step still in progress. */
export function foldRows(items: HarnessItem[], live = false): ViewRow[] {
  const out: ViewRow[] = [];
  let step: HarnessItem[] = [];
  const close = (last = false) => {
    if (step.length >= 2 && !(last && live)) out.push({ kind: "bundle", id: `b-${step[0].id}`, items: step });
    else for (const item of step) out.push({ kind: "item", item, dim: false });
    step = [];
  };
  for (const item of items) {
    if (item.kind === "assistant") {
      close();
      out.push({ kind: "item", item, dim: false });
    } else step.push(item);
  }
  close(true);
  return out;
}

/** Open while the run works, and after a stop or failure: what went wrong is
 *  in the work. Closed once it completes, and so is a stretch a steer ended. */
export const foldOpens = (state: RunState) => state !== "completed";

/** "Worked for 12s" from the run's stored interval; "Work details" without
 *  one. A stretch a restart steer superseded says it was cut short. */
export function foldLabel(fold: Pick<WorkFold, "state" | "startedMs" | "endedMs" | "cut">): string {
  if (fold.startedMs == null || fold.endedMs == null) return fold.cut === "restart" ? "Restarted" : "Work details";
  const t = elapsedText(Math.max(0, fold.endedMs - fold.startedMs));
  if (fold.cut === "restart") return `Restarted after ${t}`;
  if (fold.state === "interrupted") return `You stopped after ${t}`;
  return fold.state === "failed" ? `Failed after ${t}` : `Worked for ${t}`;
}

const WIDGET_FENCE = /(`{3,}|~{3,})[ \t]*(?:openui|widget)\b[\s\S]*?(?:\n\1|$)/g;
const squash = (s: string) => s.replace(/\s+/g, " ").trim().toLowerCase();

/** A fold's replies as find reads them once it opens: prose, without widget
 *  fences or markdown marks. Tool and reasoning rows stay closed when it
 *  opens, so their text is not searched. */
export function foldFindText(items: HarnessItem[]): string {
  return items
    .filter((i) => i.kind === "assistant")
    .map((i) => squash((i.content ?? "").replace(WIDGET_FENCE, " ").replace(/[*_`#>~|]+/g, "")))
    .join("\n");
}

/** Whether a find for `query` should open a fold whose text is `text`. */
export const foldMatches = (text: string, query: string) => {
  const q = squash(query);
  return !!q && text.includes(q);
};

/** Every item a row stands for, folded ones included: what the row window,
 *  rail jumps and find match against. */
export function rowItems(row: ViewRow): HarnessItem[] {
  return row.kind === "item" || row.kind === "artifacts" ? [row.item] : row.items;
}

function plural(n: number, one: string, many = `${one}s`) {
  return `${n} ${n === 1 ? one : many}`;
}

/** A bundle's glyph: a tool kind's, an oculus command's, or none for thoughts alone. */
export type BundleIcon = { tool: ToolKind } | { cli: CliIcon } | null;

/** One clause of a bundle's summary: what it counts, how many, its rank
 *  (0 effects, 1 reads and searches, 2 the rest) and where it was first seen. */
interface Clause {
  text: (n: number) => string;
  n: number;
  tier: 0 | 1 | 2;
  first: number;
  icon: BundleIcon;
}

const TOOL_CLAUSE: Record<ToolKind, { tier: 0 | 1 | 2; text: (n: number) => string; key?: string }> = {
  bash: { tier: 0, text: (n) => `ran ${plural(n, "command")}` },
  edit: { tier: 0, key: "edit", text: (n) => `edited ${plural(n, "file")}` },
  write: { tier: 0, key: "edit", text: (n) => `edited ${plural(n, "file")}` },
  task: { tier: 0, text: (n) => `ran ${plural(n, "subagent")}` },
  read: { tier: 1, text: (n) => `explored ${plural(n, "file")}` },
  search: { tier: 1, text: (n) => plural(n, "search", "searches") },
  web: { tier: 1, text: (n) => plural(n, "web lookup") },
  plan: { tier: 2, text: () => "updated the plan" },
  other: { tier: 2, text: (n) => plural(n, "tool call") },
  // Each oculus command is its own clause (below); this is never read.
  oculus_cli: { tier: 2, text: (n) => plural(n, "tool call") },
};

/** How many clauses a summary names before "performed N other actions". */
const SHOWN_CLAUSES = 2;

/**
 * A bundle's summary: one count per category, the two that rank highest
 * (effects — commands, edits, `oculus` writes — then reads and searches,
 * then the rest) in the order first seen, then "performed N other actions"
 * for everything else, joined with "and": "Ran 3 commands and edited 2
 * files". Reasoning is not counted. Thought-only and reply-only bundles keep
 * their own labels.
 */
export function bundleLabel(items: HarnessItem[]): { label: string; icon: BundleIcon } {
  const clauses = new Map<string, Clause>();
  let replies = 0;
  let thoughts = 0;
  const add = (key: string, at: number, c: Omit<Clause, "n" | "first">) => {
    const hit = clauses.get(key);
    if (hit) hit.n += 1;
    else clauses.set(key, { ...c, n: 1, first: at });
  };
  items.forEach((i, at) => {
    if (i.kind === "assistant") {
      replies += 1;
      add("reply", at, { tier: 2, icon: null, text: (n) => plural(n, "earlier reply", "earlier replies") });
      return;
    }
    if (i.kind === "thinking") {
      thoughts += 1;
      return;
    }
    const kind = parseToolMeta(i).kind ?? "other";
    if (kind !== "oculus_cli") {
      const c = TOOL_CLAUSE[kind];
      add(c.key ?? kind, at, { tier: c.tier, text: c.text, icon: { tool: kind } });
      return;
    }
    // Oculus commands tally by what they did; a tally that reads the same
    // ("listed memories" twice) is one clause.
    const actions = describeOculusCli(i.content ?? "");
    if (!actions.length) add("cli:other", at, { tier: 2, icon: { tool: "oculus_cli" }, text: (n) => plural(n, "tool call") });
    for (const action of actions) {
      add(`cli:${action.tally(1)}`, at, { tier: action.writes ? 0 : 1, text: action.tally, icon: { cli: action.icon } });
    }
  });
  if (replies > 0 && replies === items.length) {
    return { label: replies === 1 ? "Earlier reply" : `${replies} earlier replies`, icon: null };
  }
  if (thoughts === items.length) {
    const ms = items.map(thoughtMs);
    const total = ms.every((m) => m != null) ? ms.reduce<number>((a, m) => a + (m ?? 0), 0) : null;
    return { label: thoughtLabel(total), icon: null };
  }
  const ranked = [...clauses.values()].sort((a, b) => a.tier - b.tier || a.first - b.first);
  const shown = ranked.slice(0, SHOWN_CLAUSES);
  const others = ranked.slice(SHOWN_CLAUSES).reduce((sum, c) => sum + c.n, 0);
  const parts = [...shown].sort((a, b) => a.first - b.first).map((c) => c.text(c.n));
  if (others) parts.push(`performed ${others} other ${others === 1 ? "action" : "actions"}`);
  const label = parts.length > 1 ? `${parts.slice(0, -1).join(", ")} and ${parts[parts.length - 1]}` : (parts[0] ?? "");
  const icon: BundleIcon = ranked.find((c) => c.icon)?.icon ?? { tool: "other" };
  return { label: label.charAt(0).toUpperCase() + label.slice(1), icon };
}

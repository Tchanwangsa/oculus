import { mayHoldWidget } from "@/components/markdown/openui/remarkWidgets";
import {
  describeOculusCli,
  elapsedText,
  runEnded,
  parseToolMeta,
  thoughtLabel,
  thoughtMs,
  type CliAction,
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

/** One run's work above its answer: earlier replies, reasoning and tools. */
export interface WorkFold {
  kind: "work";
  id: string;
  thread: number;
  run: number;
  items: HarnessItem[];
  state: RunState;
  startedMs: number | null;
  endedMs: number | null;
}

export type ViewRow =
  /** `asked`: for a reply, the student's question it answers (Retry).
   *  `key`: a run's reply is keyed by its place in the run (`replyKey`). */
  | { kind: "item"; item: HarnessItem; dim: boolean; role?: ReplyRole; asked?: HarnessItem; key?: string }
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

/** One run's rows (docs/harness.md, "A run's work folds above its answer"). */
function runRows(
  segment: HarnessItem[],
  run: HarnessRun | undefined,
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
  let answer: HarnessItem | undefined;
  if (state === "running") {
    answer = streaming ? undefined : [...rest].reverse().find((i) => i.kind === "assistant");
  } else {
    answer = rest.find((i) => i.id === run!.answer_item_id) ?? rest.find((i) => i.display_role === "answer");
  }
  const work = rest.filter((i) => i !== answer && (i.kind === "tool" || i.kind === "thinking" || i.kind === "assistant"));
  const folded = new Set(work);
  if (head) out.push({ kind: "item", item: head, dim: false });
  if (work.length) {
    out.push({
      kind: "work",
      id: `w-${run?.id ?? segment[0].id}`,
      thread: segment[0].thread_id,
      run: run?.id ?? segment[0].run_id!,
      items: work,
      state,
      startedMs: run?.started_ms ?? null,
      endedMs: state === "running" ? null : (run?.ended_ms ?? null),
    });
    for (const item of work) {
      if (item.kind === "assistant" && mayHoldWidget(item.content)) out.push({ kind: "artifacts", id: `a-${item.id}`, item });
    }
  }
  const role: ReplyRole = state === "running" ? "provisional" : state === "completed" ? "answer" : "partial";
  let replies = 0;
  for (const item of rest) {
    const key = item.kind === "assistant" ? replyKey(item.run_id!, replies++) : undefined;
    if (item === answer) out.push({ kind: "item", item, dim: false, role, ...(head?.kind === "user" && { asked: head }), ...(key && { key }) });
    else if (!folded.has(item)) out.push({ kind: "item", item, dim: false });
  }
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
 * The thread as rows. Each run's contiguous rows become: its question, one
 * fold of its work, the widget fences of folded replies, then its answer and
 * outcome rows (notice, stop, error, refusal, compaction) in order. Rows with
 * no run keep the step bundles. `running` is the thread's: only its last run
 * can be in flight.
 */
export function buildRows(items: HarnessItem[], running: boolean, ctx: RowContext = {}): ViewRow[] {
  const out: ViewRow[] = [];
  const runs = ctx.runs ?? {};
  let i = 0;
  while (i < items.length) {
    const runId = items[i].run_id;
    let j = i + 1;
    if (runId == null) {
      while (j < items.length && items[j].run_id == null) j++;
      stepRows(items.slice(i, j), running && j === items.length, out);
    } else {
      while (j < items.length && items[j].run_id === runId) j++;
      runRows(items.slice(i, j), runs[runId], running && j === items.length, !!ctx.streaming, out);
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
 *  in the work. Closed once it completes. */
export const foldOpens = (state: RunState) => state !== "completed";

/** "Worked for 12s" from the run's stored interval; "Work details" without one. */
export function foldLabel(fold: Pick<WorkFold, "state" | "startedMs" | "endedMs">): string {
  if (fold.startedMs == null || fold.endedMs == null) return "Work details";
  const t = elapsedText(Math.max(0, fold.endedMs - fold.startedMs));
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

/** "Explored 3 files, saved 2 memories, ran 1 command" */
export function bundleLabel(items: HarnessItem[]): { label: string; icon: BundleIcon } {
  const counts: Partial<Record<ToolKind | "thinking", number>> = {};
  // Oculus commands tally by what they did, in the order first seen.
  const cli = new Map<string, { action: CliAction; n: number }>();
  let replies = 0;
  for (const i of items) {
    if (i.kind === "assistant") {
      replies += 1;
      continue;
    }
    const k = i.kind === "thinking" ? "thinking" : (parseToolMeta(i).kind ?? "other");
    counts[k] = (counts[k] ?? 0) + 1;
    if (k !== "oculus_cli") continue;
    for (const action of describeOculusCli(i.content ?? "")) {
      const hit = cli.get(action.key);
      if (hit) hit.n += 1;
      else cli.set(action.key, { action, n: 1 });
    }
  }
  if (replies > 0 && replies === items.length) {
    return { label: replies === 1 ? "Earlier reply" : `${replies} earlier replies`, icon: null };
  }
  if (counts.thinking === items.length) {
    const ms = items.map(thoughtMs);
    const total = ms.every((m) => m != null) ? ms.reduce<number>((a, m) => a + (m ?? 0), 0) : null;
    return { label: thoughtLabel(total), icon: null };
  }
  const parts: string[] = [];
  const reads = counts.read ?? 0;
  if (reads) parts.push(`explored ${plural(reads, "file")}`);
  if (counts.search) parts.push(plural(counts.search, "search", "searches"));
  // A tally merges clauses that read the same ("listed memories" twice).
  for (const clause of new Set([...cli.values()].map(({ action, n }) => action.tally(n)))) parts.push(clause);
  if (counts.bash) parts.push(`ran ${plural(counts.bash, "command")}`);
  const edits = (counts.edit ?? 0) + (counts.write ?? 0);
  if (edits) parts.push(`edited ${plural(edits, "file")}`);
  if (counts.web) parts.push(plural(counts.web, "web lookup"));
  if (counts.task) parts.push(`ran ${plural(counts.task, "subagent")}`);
  if (counts.plan) parts.push("updated the plan");
  if (counts.other) parts.push(plural(counts.other, "tool call"));
  if (replies) parts.push(plural(replies, "earlier reply", "earlier replies"));
  const label = parts.join(", ");
  const dominant = (Object.entries(counts) as [ToolKind | "thinking", number][])
    .filter(([k]) => k !== "thinking")
    .sort((a, b) => b[1] - a[1])[0]?.[0] as ToolKind | undefined;
  const topCli = [...cli.values()].sort((a, b) => b.n - a.n)[0]?.action.icon;
  const icon: BundleIcon = dominant === "oculus_cli" && topCli ? { cli: topCli } : { tool: dominant ?? "other" };
  return { label: label.charAt(0).toUpperCase() + label.slice(1), icon };
}

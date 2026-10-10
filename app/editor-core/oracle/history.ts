// Checks the Rust `History` against @codemirror/commands' `history()` on an
// `EditorState` with multiple selections (as the app's `drawSelection`).
// Each case is a random document and a random sequence of steps, each with a
// controlled `Transaction.time`: typing through `replaceSelection`
// (`input.type`, `input.type.compose`), deleting (`delete.backward`/
// `.forward`), pastes and completions (some `isolateHistory`), selection-only
// transactions (`select`, `select.pointer`), `addToHistory: false` writes that
// rebase the stacks, no-op isolations, and undo/redo/undoSelection/
// redoSelection, sometimes run to exhaustion. After every step the document,
// selection, depths and the whole history (both branches' events, prevTime,
// prevUserEvent) are compared; long sequences compare digests of the document
// and history between full comparisons every 20 steps and at the end. Then every corpus note is typed in character
// by character with random undos, redos and cursor moves.
//
//   bun editor-core/oracle/history.ts [cases] [seed] [only]     (from app/)

import {
  ChangeSet,
  EditorSelection,
  EditorState,
  Transaction,
  type Text,
  type TransactionSpec,
} from "@codemirror/state";
import {
  history,
  historyField,
  isolateHistory,
  redo,
  redoDepth,
  redoSelection,
  undo,
  undoDepth,
  undoSelection,
} from "@codemirror/commands";

import { selectionJson } from "./changes";
import { RUNS, boundary, isLowSurrogate, source } from "./docs";
import { Checker, type Rng, buildOracle, caseRng, corpus, parseArgs } from "./driver";

const args = parseArgs(20000);
const script = "bun editor-core/oracle/history.ts";

type Spec = [number, number, string];
type RangeSpec = [number, number, number | null, number | null, number];
type SelSpec = { ranges: RangeSpec[]; main: number };
type Isolate = "before" | "after" | "full";

const extensions = [history(), EditorState.allowMultipleSelections.of(true)];

function selSpec(sel: EditorSelection): SelSpec {
  return {
    ranges: sel.ranges.map((r) => [r.anchor, r.head, r.goalColumn ?? null, r.bidiLevel, r.assoc]),
    main: sel.mainIndex,
  };
}

function cmSelection(s: SelSpec): EditorSelection {
  return EditorSelection.create(
    s.ranges.map(([a, h, goal, bidi, assoc]) =>
      EditorSelection.range(a, h, goal ?? undefined, bidi ?? undefined, assoc || undefined),
    ),
    s.main,
  );
}

function randomSelection(rng: Rng, doc: Text): SelSpec {
  const n = rng.pick([1, 1, 1, 1, 2, 3]);
  const ranges: RangeSpec[] = [];
  for (let i = 0; i < n; i++) {
    const anchor = boundary(rng, doc);
    let head = rng.chance(0.6) ? anchor : Math.min(doc.length, anchor + rng.int(6));
    if (isLowSurrogate(doc, head)) head--;
    const goal = rng.chance(0.1) ? rng.pick([0, 7.5, 40]) : null;
    const bidi = rng.chance(0.05) ? rng.int(3) : null;
    const assoc = rng.chance(0.2) ? rng.pick([-1, 1]) : 0;
    ranges.push([rng.chance(0.2) ? head : anchor, rng.chance(0.2) ? anchor : head, goal, bidi, assoc]);
  }
  // Round-trip through CodeMirror so the spec is already normalised.
  return selSpec(cmSelection({ ranges, main: rng.int(n) }));
}

/** Specs on `doc`, ascending, near `around` (a cursor) when given. */
function randomSpecs(rng: Rng, doc: Text, around: number | null): Spec[] {
  const n = rng.pick([1, 1, 2, 3]);
  const out: Spec[] = [];
  let pos = around === null ? 0 : Math.max(0, around - rng.int(10));
  for (let i = 0; i < n; i++) {
    let from = around === null && i === 0 ? boundary(rng, doc) : Math.min(doc.length, pos + rng.int(6));
    if (isLowSurrogate(doc, from)) from--;
    if (from < pos) break;
    let to = Math.min(doc.length, from + rng.pick([0, 0, 1, 3, 10, 40]));
    if (isLowSurrogate(doc, to)) to++;
    const insert = rng.chance(0.25) ? "" : source(rng, rng.logInt(1, 8), 0.1);
    if (from === to && insert === "") continue;
    out.push([from, to, insert]);
    pos = to;
  }
  return out;
}

/** The previous (or next) code point's start (or end) from `pos`. */
function stepCodePoint(doc: Text, pos: number, forward: boolean): number {
  if (forward) {
    if (pos >= doc.length) return pos;
    return isLowSurrogate(doc, pos + 1) ? pos + 2 : pos + 1;
  }
  if (pos <= 0) return pos;
  return isLowSurrogate(doc, pos - 1) ? pos - 2 : pos - 1;
}

function eventJson(e: any) {
  const o: Record<string, unknown> = {};
  if (e.changes) o.changes = e.changes.toJSON();
  if (e.mapped) o.mapped = e.mapped.toJSON();
  if (e.startSelection) o.start = selectionJson(e.startSelection);
  o.after = e.selectionsAfter.map(selectionJson);
  return o;
}

function snapshot(state: EditorState, ran: boolean, full: boolean) {
  const out: Record<string, unknown> = {
    ran,
    sel: selectionJson(state.selection),
    undo_depth: undoDepth(state),
    redo_depth: redoDepth(state),
    prev_time: (state.field(historyField) as any).prevTime,
  };
  const h = state.field(historyField) as any;
  const hist = JSON.parse(
    JSON.stringify({
      done: h.done.map(eventJson),
      undone: h.undone.map(eventJson),
      prev_time: h.prevTime,
      prev_user_event: h.prevUserEvent,
    }),
  );
  if (full) {
    out.doc = state.doc.toString();
    out.hist = hist;
  } else {
    out.doc_fnv = fnv1a(state.doc.toString());
    out.hist_digest = fnv1a(canonical(hist));
  }
  return out;
}

/** FNV-1a (32-bit) over UTF-16 units, as src/bin/oracle/text.rs. */
function fnv1a(s: string): number {
  let h = 0x811c9dc5;
  for (let i = 0; i < s.length; i++) h = Math.imul(h ^ s.charCodeAt(i), 0x01000193);
  return h >>> 0;
}

/** The canonical form `digest` in src/bin/oracle/history.rs hashes. */
function canonical(v: unknown): string {
  if (v === null) return "n";
  if (typeof v === "boolean") return v ? "t" : "f";
  if (typeof v === "number") return String(v);
  if (typeof v === "string") return `s${v.length}:${v}`;
  if (Array.isArray(v)) return `[${v.map((x) => canonical(x) + ",").join("")}]`;
  const o = v as Record<string, unknown>;
  return `{${Object.keys(o).sort().map((k) => `${k}:${canonical(o[k])},`).join("")}}`;
}

/** Runs one request step on the CodeMirror side; returns whether it ran. */
function cmStep(state: EditorState, step: any): { state: EditorState; ran: boolean } {
  const annotations = [Transaction.time.of(step.time)];
  if (step.isolate) annotations.push(isolateHistory.of(step.isolate));
  if (step.kind === "tr") {
    if (step.add_to_history === false) annotations.push(Transaction.addToHistory.of(false));
    const spec: TransactionSpec = {
      changes: step.changes.map(([from, to, insert]: Spec) => ({ from, to, insert })),
      annotations,
    };
    if (step.selection) spec.selection = cmSelection(step.selection);
    if (step.user_event !== undefined) spec.userEvent = step.user_event;
    return { state: state.update(spec).state, ran: true };
  }
  if (step.kind === "replace_selection") {
    const spec: TransactionSpec = { ...state.replaceSelection(step.text), annotations };
    if (step.user_event !== undefined) spec.userEvent = step.user_event;
    return { state: state.update(spec).state, ran: true };
  }
  const command = { undo, redo, undo_selection: undoSelection, redo_selection: redoSelection }[step.kind as string]!;
  let next = state;
  const ran = command({ state, dispatch: (tr: Transaction) => (next = tr.state) });
  return { state: next, ran };
}

const TYPED = [...RUNS.filter((r) => [...r].length === 1), "\n", "x", "e", " "];

/** One random step for `state`. */
function randomStep(rng: Rng, state: EditorState, time: number): Record<string, unknown> {
  const doc = state.doc;
  const r = rng.next();
  const isolate = (p: number): Isolate | undefined => (rng.chance(p) ? rng.pick(["before", "after", "full"] as const) : undefined);
  if (r < 0.33) {
    const user_event = rng.chance(0.8)
      ? "input.type"
      : rng.pick(["input.type.compose", "input.type.compose.start", "input.typex", "input", "", undefined]);
    return { kind: "replace_selection", text: rng.pick(TYPED), user_event, isolate: isolate(0.03), time };
  }
  if (r < 0.45) {
    const forward = rng.chance(0.25);
    const specs: Spec[] = [];
    const out = state.changeByRange((range) => {
      if (!range.empty) {
        specs.push([range.from, range.to, ""]);
        return { changes: { from: range.from, to: range.to }, range: EditorSelection.cursor(range.from) };
      }
      const other = stepCodePoint(doc, range.head, forward);
      if (other === range.head) return { range };
      const [from, to] = [Math.min(other, range.head), Math.max(other, range.head)];
      specs.push([from, to, ""]);
      return { changes: { from, to }, range: EditorSelection.cursor(from) };
    });
    return {
      kind: "tr",
      changes: specs,
      selection: selSpec(out.selection),
      user_event: forward ? "delete.forward" : rng.chance(0.85) ? "delete.backward" : rng.pick(["delete", "deletex", "delete.cut"]),
      time,
    };
  }
  if (r < 0.55) {
    const specs = randomSpecs(rng, doc, rng.chance(0.5) ? state.selection.main.head : null);
    const newDoc = ChangeSet.of(specs.map(([from, to, insert]) => ({ from, to, insert })), doc.length).apply(doc);
    const user_event = rng.pick(["input.paste", "input", "input.complete", "input.type", undefined]);
    const iso = user_event === "input.complete" && rng.chance(0.7) ? "full" : isolate(0.15);
    return {
      kind: "tr",
      changes: specs,
      selection: rng.chance(0.6) ? randomSelection(rng, newDoc) : undefined,
      user_event,
      isolate: iso,
      time,
    };
  }
  if (r < 0.67) {
    return {
      kind: "tr",
      changes: [],
      selection: randomSelection(rng, doc),
      user_event: rng.pick(["select", "select", "select.pointer", "selectx", "select.undo", undefined, "", "input"]),
      time,
    };
  }
  if (r < 0.76) {
    // Untracked writes, as the maths field's live edits: near the cursor,
    // or anywhere, sometimes swallowing recent edits.
    const specs = randomSpecs(rng, doc, rng.chance(0.6) ? state.selection.main.head : null);
    if (rng.chance(0.2) && doc.length > 0) {
      let from = boundary(rng, doc);
      let to = Math.min(doc.length, from + rng.logInt(1, 60));
      if (isLowSurrogate(doc, to)) to++;
      specs.length = 0;
      specs.push([from, to, rng.chance(0.5) ? "" : "$y$"]);
    }
    return {
      kind: "tr",
      changes: specs,
      add_to_history: false,
      isolate: isolate(0.1),
      user_event: rng.chance(0.3) ? "input.type" : undefined,
      time,
    };
  }
  if (r < 0.78) return { kind: "tr", changes: [], isolate: isolate(1), time };
  if (r < 0.88) return { kind: "undo", time };
  if (r < 0.94) return { kind: "redo", time };
  if (r < 0.97) return { kind: "undo_selection", time };
  return { kind: "redo_selection", time };
}

/** The next transaction time: mostly inside the 500 ms group window. */
function nextTime(rng: Rng, time: number): number {
  const r = rng.next();
  if (r < 0.02) return Math.max(0, time - rng.int(1000));
  return time + rng.pick([0, 1, 30, 120, 300, 499, 500, 501, 800, 5000]);
}

function randomCase(rng: Rng) {
  const raw = rng.chance(0.05) ? source(rng, rng.logInt(100, 2000), 0.05) : source(rng, rng.int(30), 0.15);
  let state = EditorState.create({ doc: raw, extensions });
  const selection = randomSelection(rng, state.doc);
  state = state.update({ selection: cmSelection(selection) }).state;
  // A fresh state's history is empty; the selection above is not recorded.
  state = EditorState.create({ doc: state.doc, selection: state.selection, extensions });
  let time = rng.pick([0, 0, 1000, 1.7e12 + rng.int(1e9)]);
  const steps: Record<string, unknown>[] = [];
  const expected: unknown[] = [];
  // Rarely long enough to trim a branch (`minDepth` 100 + 20) or fill an
  // event's selections (200).
  const n = rng.chance(0.02) ? 400 : rng.pick([1, 3, 8, 20, 40, 80]);
  const storm = n === 400 && rng.chance(0.5);
  // Long sequences snapshot the whole history every 20 steps, else digests.
  const run = (step: Record<string, unknown>) => {
    const full = n <= 80 || steps.length % 20 === 0;
    let r;
    try {
      r = cmStep(state, step);
    } catch {
      // CodeMirror throws (replaceSelection over a range whose `from > to`,
      // which mapping can leave); `State::replace_selection` refuses it too.
      // The step is left out.
      refused++;
      return false;
    }
    state = r.state;
    step.full = full;
    steps.push(step);
    expected.push(snapshot(state, r.ran, full));
    return r.ran;
  };
  for (let i = 0; i < n; i++) {
    time = nextTime(rng, time);
    if (storm && rng.chance(0.98)) {
      run({ kind: "tr", changes: [], selection: randomSelection(rng, state.doc), user_event: rng.pick(["select", undefined]), time });
      continue;
    }
    if (rng.chance(0.03)) {
      // Undo to exhaustion, then redo back.
      for (const kind of rng.chance(0.3) ? ["undo_selection", "redo_selection"] : ["undo", "redo"]) {
        for (let k = 0; k < 300 && run({ kind, time }); k++) time = nextTime(rng, time);
      }
      continue;
    }
    let step;
    try {
      step = randomStep(rng, state, time);
    } catch {
      refused++; // changeByRange over a range with `from > to`; see `run`
      continue;
    }
    run(step);
  }
  // The last step always compares in full.
  if (steps.length) {
    steps[steps.length - 1].full = true;
    const last = expected.length - 1;
    expected[last] = snapshot(state, (expected[last] as { ran: boolean }).ran, true);
  }
  const request = { op: "history", doc: raw, selection, steps };
  return { request, expected: { steps: expected } };
}

/** Types `source` in at the cursor one code point at a time, with random
 * undos, redos and cursor moves; full snapshots every 100 steps. */
function corpusCase(rng: Rng, raw: string) {
  let state = EditorState.create({ doc: "", extensions });
  const steps: Record<string, unknown>[] = [];
  const expected: unknown[] = [];
  let time = 1.7e12;
  const text = raw.replace(/\r\n?/g, "\n");
  const run = (step: Record<string, unknown>) => {
    const r = cmStep(state, step);
    state = r.state;
    step.full = steps.length % 100 === 99;
    steps.push(step);
    expected.push(snapshot(state, r.ran, step.full as boolean));
  };
  for (const ch of text) {
    time += rng.chance(0.05) ? 600 + rng.int(3000) : 20 + rng.int(250);
    const r = rng.next();
    if (r < 0.03) run({ kind: "undo", time });
    else if (r < 0.045) run({ kind: "redo", time });
    else if (r < 0.055) {
      const end = state.doc.length;
      run({ kind: "tr", changes: [], selection: selSpec(EditorSelection.single(end)), user_event: "select", time });
    }
    run({ kind: "replace_selection", text: ch, user_event: ch === "\n" ? "input" : "input.type", time });
  }
  steps[steps.length - 1].full = true;
  expected[expected.length - 1] = snapshot(state, (expected[expected.length - 1] as any).ran, true);
  return { request: { op: "history", doc: "", selection: { ranges: [[0, 0, null, null, 0]], main: 0 }, steps }, expected: { steps: expected } };
}

const binary = buildOracle();
const checker = new Checker("history oracle", binary, args.seed);
let count = 0;
let stepCount = 0;
let refused = 0;
for (let i = 0; i < args.cases; i++) {
  if (args.only !== null && i !== args.only) continue;
  const { request, expected } = randomCase(caseRng(args.seed, i));
  stepCount += request.steps.length;
  checker.add(`random #${i}`, `${script} ${args.cases} ${args.seed} ${i}`, request, expected);
  count++;
}
let notes = 0;
if (args.only === null) {
  for (const [j, note] of (await corpus()).entries()) {
    const { request, expected } = corpusCase(caseRng(args.seed, 1e9 + j), note.source);
    stepCount += request.steps.length;
    checker.add(`corpus ${note.name}`, `${script} ${args.cases} ${args.seed}`, request, expected);
    notes++;
  }
}
checker.finish(`${count} random sequences + ${notes} corpus notes, ${stepCount} steps (${refused} refused by CodeMirror, skipped)`, count);

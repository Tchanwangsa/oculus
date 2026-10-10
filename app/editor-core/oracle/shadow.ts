// Runs the app's shadow mode (src/components/documents/editor/shadow/) headless:
// `EditorState`s with the real `history()`, `noteLanguage` and the shadow's
// field, driven by seeded random transactions — typing runs inside and around
// the 500 ms group window, pastes of Thai, emoji and CRLF text, deletes,
// selection moves, `replaceSelection`, untracked whole-document rewrites,
// `isolateHistory`, and the real undo/redo/undoSelection/redoSelection
// commands, some states with a transaction filter that moves selections —
// over plain and markdown-heavy documents. States start fresh,
// restored through `historyField` JSON, or seeded late (the wasm "loads" after
// a few transactions, so the shadow seeds from a live mid-session state).
// After every transaction the field's own checks run, then the view's depth
// checks; every few steps the full text, history and tree check. Any report
// fails the run. Each case ends by replaying its report payload through a
// fresh `Shadow` and comparing it with CodeMirror. Self-tests first corrupt
// the shadow on purpose and require exactly one report, so a silent shadow
// cannot pass. Corpus notes (read at run time) get a case each.
//
//   bun editor-core/oracle/shadow.ts [cases] [seed] [only]     (from app/)

import { resolve, join } from "node:path";

import { history, historyField, isolateHistory, redo, redoSelection, undo, undoSelection } from "@codemirror/commands";
import { LanguageSupport, ensureSyntaxTree } from "@codemirror/language";
import {
  EditorSelection,
  EditorState,
  Transaction,
  type Annotation,
  type Extension,
  type TransactionSpec,
} from "@codemirror/state";

import { noteLanguage } from "../../src/components/documents/editor/core/language";
import { createEditorShadow, type EditorShadow } from "../../src/components/documents/editor/shadow/core";
import { firstDifference } from "../../src/components/documents/editor/shadow/diff";
import { runReplay } from "../../src/components/documents/editor/shadow/replay";
import type { MismatchKind, ShadowWasm, WasmShadow } from "../../src/components/documents/editor/shadow/types";
import { RUNS, boundary, isLowSurrogate, source } from "./docs";
import { type Rng, caseRng, corpus, parseArgs } from "./driver";
import { edges, mutated, soup, structured, table } from "./markdown-docs";

const args = parseArgs(300);
const script = "bun editor-core/oracle/shadow.ts";
const app = resolve(import.meta.dir, "../..");

const build = Bun.spawnSync([process.execPath, "scripts/build-editor-wasm.mjs"], {
  cwd: app,
  stdout: "inherit",
  stderr: "inherit",
});
if (build.exitCode !== 0) {
  console.error("shadow oracle: the editor core's wasm did not build (see above)");
  process.exit(build.exitCode ?? 1);
}
const pkg = join(app, "src/components/documents/editor/shadow/pkg");
const glue = await import(join(pkg, "oculus_editor_core_wasm.js"));
glue.initSync({ module: await Bun.file(join(pkg, "oculus_editor_core_wasm_bg.wasm")).arrayBuffer() });
const real: ShadowWasm = {
  seed: (doc, selection, history) => glue.Shadow.seed(doc, selection, history),
  nodeNames: glue.Shadow.nodeNames(),
};

interface Report {
  kind: MismatchKind | "error";
  details: unknown;
}

function collector(into: Report[]) {
  return {
    mismatch: (kind: MismatchKind, details: Record<string, unknown>) => into.push({ kind, details }),
    error: (e: unknown) => into.push({ kind: "error", details: e instanceof Error ? e.stack ?? e.message : e }),
  };
}

const plain = (s: EditorSelection) => JSON.stringify(s.toJSON());
const historyOf = (s: EditorState) => JSON.stringify(s.toJSON({ history: historyField }).history);

/** Drives one chain of states through `shadow`, checking as the app does. */
class Driver {
  steps = 0;
  fullChecks = 0;
  treeChecks = 0;

  constructor(
    public state: EditorState,
    readonly shadow: EditorShadow,
    public time: number,
    /** The wasm "loads" once this many transactions have run; null: loaded. */
    readonly lateAt: number | null = null,
  ) {}

  private before(steps: number) {
    if (this.lateAt !== null && steps >= this.lateAt) wasmNow = real;
  }

  /** One transaction from `spec`, as dispatched. */
  tr(spec: TransactionSpec, annotations: Annotation<unknown>[] = []) {
    this.before(this.steps);
    const tr = this.state.update(spec, { annotations: [Transaction.time.of(this.time), ...annotations] });
    this.land(tr);
  }

  /** A history command; false when it had nothing to do. */
  command(cmd: typeof undo): boolean {
    this.before(this.steps);
    let captured = null as Transaction | null;
    const ran = cmd({ state: this.state, dispatch: (tr) => (captured = tr) });
    if (captured) this.land(captured);
    return ran;
  }

  private land(tr: Transaction) {
    this.state = tr.state;
    this.steps++;
    this.shadow.check(this.state);
    if (this.steps % 7 === 0) this.full();
  }

  full() {
    const tree = ensureSyntaxTree(this.state, this.state.doc.length, 1e9);
    this.fullChecks++;
    if (tree && tree.length === this.state.doc.length) this.treeChecks++;
    this.shadow.check(this.state, { full: true, tree });
  }
}

const TYPED = [...RUNS.filter((r) => [...r].length === 1), "\n", "x", "e", " ", "*", "`", "#", "-", "$"];

/** The next transaction time: mostly inside the 500 ms group window. */
function nextTime(rng: Rng, time: number): number {
  if (rng.chance(0.02)) return Math.max(0, time - rng.int(1000));
  return time + rng.pick([0, 1, 30, 120, 300, 499, 500, 501, 800, 5000]);
}

function randomDoc(rng: Rng): string {
  const r = rng.next();
  if (r < 0.25) return source(rng, rng.int(60), 0.15);
  if (r < 0.6) return structured(rng);
  if (r < 0.7) return table(rng);
  if (r < 0.8) return edges(rng);
  if (r < 0.9) return mutated(rng);
  return soup(rng);
}

function randomSelection(rng: Rng, state: EditorState): EditorSelection {
  const doc = state.doc;
  const multi = state.facet(EditorState.allowMultipleSelections);
  const n = multi ? rng.pick([1, 1, 1, 2, 3]) : 1;
  const ranges = [];
  for (let i = 0; i < n; i++) {
    const anchor = boundary(rng, doc);
    let head = rng.chance(0.6) ? anchor : Math.min(doc.length, anchor + rng.int(8));
    if (isLowSurrogate(doc, head)) head--;
    const goal = rng.chance(0.15) ? rng.pick([0, 3, 40]) : undefined;
    const assoc = rng.chance(0.2) ? rng.pick([-1, 1] as const) : undefined;
    ranges.push(
      anchor === head
        ? EditorSelection.cursor(anchor, assoc, undefined, goal)
        : EditorSelection.range(anchor, head, goal, undefined, assoc),
    );
  }
  return EditorSelection.create(ranges, rng.int(n));
}

/** The previous (or next) code point's start (or end) from `pos`. */
function stepCodePoint(state: EditorState, pos: number, forward: boolean): number {
  const doc = state.doc;
  if (forward) return pos >= doc.length ? pos : isLowSurrogate(doc, pos + 1) ? pos + 2 : pos + 1;
  return pos <= 0 ? pos : isLowSurrogate(doc, pos - 1) ? pos - 2 : pos - 1;
}

const isolate = (rng: Rng) => rng.pick(["before", "after", "full"] as const);

/** One random action: usually one transaction, a typing run several. */
function act(rng: Rng, d: Driver) {
  const r = rng.next();
  const s = d.state;
  if (r < 0.3) {
    for (let k = 1 + rng.int(10); k > 0; k--) {
      d.time += rng.pick([0, 15, 40, 90, 200, 499, 500, 501]);
      d.tr({ ...d.state.replaceSelection(rng.pick(TYPED)), userEvent: rng.chance(0.95) ? "input.type" : "input.type.compose" });
    }
    return;
  }
  d.time = nextTime(rng, d.time);
  if (r < 0.38) {
    const text = rng.chance(0.5) ? source(rng, rng.logInt(1, 200), 0.2) : randomDoc(rng);
    const iso = rng.chance(0.15) ? [isolateHistory.of(isolate(rng))] : [];
    d.tr({ ...s.replaceSelection(text), userEvent: "input.paste" }, iso);
  } else if (r < 0.5) {
    const forward = rng.chance(0.3);
    const spec = s.changeByRange((range) => {
      if (!range.empty) return { changes: { from: range.from, to: range.to }, range: EditorSelection.cursor(range.from) };
      const other = stepCodePoint(s, range.head, forward);
      const [from, to] = [Math.min(other, range.head), Math.max(other, range.head)];
      return { changes: { from, to }, range: EditorSelection.cursor(from) };
    });
    d.tr({ ...spec, userEvent: forward ? "delete.forward" : rng.pick(["delete.backward", "delete.backward", "delete.cut"]) });
  } else if (r < 0.62) {
    d.tr({ selection: randomSelection(rng, s), userEvent: rng.pick(["select", "select", "select.pointer", undefined]) });
  } else if (r < 0.67) {
    const event = rng.pick(["input", "input.complete", undefined]);
    const iso = event === "input.complete" && rng.chance(0.7) ? [isolateHistory.of("full" as const)] : [];
    d.tr({ ...s.replaceSelection(source(rng, rng.logInt(1, 12), 0.1)), userEvent: event }, iso);
  } else if (r < 0.71) {
    // A whole-document rewrite outside the history, as a full-doc sync.
    const doc = rng.chance(0.5) ? randomDoc(rng) : s.doc.toString() + source(rng, 5, 0.3);
    const iso = rng.chance(0.2) ? [isolateHistory.of(isolate(rng))] : [];
    d.tr({ changes: { from: 0, to: s.doc.length, insert: doc } }, [Transaction.addToHistory.of(false), ...iso]);
  } else if (r < 0.75) {
    // A small untracked edit near the caret, as another view's replayed edit.
    const at = Math.min(s.doc.length, s.selection.main.head + rng.int(5));
    const pos = isLowSurrogate(s.doc, at) ? at - 1 : at;
    d.tr({ changes: { from: pos, insert: rng.pick(RUNS) } }, [Transaction.addToHistory.of(false)]);
  } else if (r < 0.78) {
    d.tr({}, [isolateHistory.of(isolate(rng))]);
  } else if (r < 0.89) {
    d.command(undo);
  } else if (r < 0.94) {
    d.command(redo);
  } else if (r < 0.97) {
    d.command(undoSelection);
  } else if (r < 0.99) {
    d.command(redoSelection);
  } else {
    const [back, forth] = rng.chance(0.3) ? [undoSelection, redoSelection] : [undo, redo];
    for (const cmd of [back, forth]) {
      for (let k = 0; k < 300 && d.command(cmd); k++) d.time = nextTime(rng, d.time);
    }
  }
}

/** `act`, minus the few random specs CodeMirror refuses (a range mapped to
 *  `from > to`), which the shadow never sees. Anything else is a bug here. */
function tryAct(rng: Rng, d: Driver) {
  try {
    act(rng, d);
  } catch (e) {
    if (!(e instanceof RangeError)) throw e;
  }
}

let wasmNow: ShadowWasm | null = real;
const reports: Report[] = [];
const shadow = createEditorShadow({ wasm: () => wasmNow, reporter: collector(reports) });
const language = new LanguageSupport(noteLanguage);

/** Moves some selection-only transactions' selection, select.undo and
 *  select.redo included, as the maths field's `fieldSelection` filter does. */
const selectionFilter = EditorState.transactionFilter.of((tr) => {
  if (tr.docChanged || !tr.selection || !tr.isUserEvent("select") || tr.newDoc.length % 3 !== 0) return tr;
  const mid = tr.newDoc.length >> 1;
  return [tr, { selection: EditorSelection.cursor(isLowSurrogate(tr.newDoc, mid) ? mid - 1 : mid), sequential: true }];
});

function extensionsFor(rng: Rng, withShadow: boolean): Extension[] {
  const out: Extension[] = [history(), language];
  if (rng.chance(0.2)) out.push(EditorState.allowMultipleSelections.of(true));
  if (rng.chance(0.25)) out.push(selectionFilter);
  if (withShadow) out.push(shadow.extension);
  return out;
}

type Start = "fresh" | "restored" | "late";

/** A start state and how it began. */
function start(rng: Rng, doc: string, actions: number): { driver: Driver; how: Start } {
  const how = rng.pick(["fresh", "fresh", "restored", "late"] as const);
  const exts = extensionsFor(rng, true);
  let time = rng.pick([0, 1000, 1.7e12 + rng.int(1e9)]);
  if (how === "restored") {
    // Random steps without the shadow, then a round trip through JSON, as a
    // remount restores a note's state.
    const plainExts = exts.slice(0, -1);
    const pre = new Driver(EditorState.create({ doc, extensions: plainExts }), shadow, time);
    for (let i = rng.pick([3, 10, 30]); i > 0; i--) tryAct(rng, pre);
    const json = pre.state.toJSON({ history: historyField });
    const state = EditorState.fromJSON(json, { extensions: exts }, { history: historyField });
    return { driver: new Driver(state, shadow, pre.time + 1000), how };
  }
  if (how === "late") {
    wasmNow = null;
    const lateAt = 1 + rng.int(Math.ceil(actions / 2));
    return { driver: new Driver(EditorState.create({ doc, extensions: exts }), shadow, time, lateAt), how };
  }
  return { driver: new Driver(EditorState.create({ doc, extensions: exts }), shadow, time), how };
}

let failures = 0;
const fail = (label: string, replay: string, message: string, details?: unknown) => {
  failures++;
  if (failures > 5) return;
  console.error(`MISMATCH ${label}: ${message}`);
  if (details !== undefined) console.error(`  ${JSON.stringify(details, null, 0).slice(0, 1500)}`);
  console.error(`  replay:   ${replay}`);
};

/** Checks one finished chain: no reports, still live, and its replay
 *  payload rebuilds a shadow equal to CodeMirror's state. */
function finish(label: string, replay: string, d: Driver) {
  d.full();
  if (reports.length) {
    const first = reports[0];
    reports.length = 0;
    fail(label, replay, `${first.kind} reported`, first.details);
    return;
  }
  const status = shadow.status(d.state);
  // Too few transactions for a late load to have happened.
  if (status === "unseeded" && d.lateAt !== null && d.steps <= d.lateAt) return;
  if (status !== "live") return fail(label, replay, `shadow is ${status} at the end`);
  const payload = shadow.replayOf(d.state)!;
  let rebuilt: WasmShadow;
  try {
    rebuilt = runReplay(real, payload);
  } catch (e) {
    return fail(label, replay, `replay payload did not run: ${e}`);
  }
  const want = [d.state.doc.toString(), plain(d.state.selection), historyOf(d.state)];
  const got = [rebuilt.text(), rebuilt.selectionJson(), rebuilt.historyJson()];
  const which = ["doc", "selection", "history"];
  for (let i = 0; i < 3; i++) {
    if (want[i] !== got[i]) return fail(label, replay, `replayed ${which[i]} differs`, firstDifference(want[i], got[i]));
  }
}

let steps = 0;
let fullChecks = 0;
let treeChecks = 0;
const starts: Record<Start, number> = { fresh: 0, restored: 0, late: 0 };

function runDriver(label: string, replay: string, rng: Rng, d: Driver, actions: number) {
  for (let i = 0; i < actions; i++) tryAct(rng, d);
  finish(label, replay, d);
  steps += d.steps;
  fullChecks += d.fullChecks;
  treeChecks += d.treeChecks;
  wasmNow = real;
}

selfTests();

let cases = 0;
for (let i = 0; i < args.cases; i++) {
  if (args.only !== null && i !== args.only) continue;
  const rng = caseRng(args.seed, i);
  const actions = rng.pick([5, 20, 60, 150]);
  const { driver, how } = start(rng, randomDoc(rng), actions);
  starts[how]++;
  runDriver(`random #${i} (${how})`, `${script} ${args.cases} ${args.seed} ${i}`, rng, driver, actions);
  cases++;
  // wasm-bindgen frees a dropped Shadow from a FinalizationRegistry callback,
  // which only runs once the event loop turns, as it always does in the app.
  await new Promise((r) => setTimeout(r, 0));
}
let notes = 0;
if (args.only === null) {
  for (const [j, note] of (await corpus()).entries()) {
    const rng = caseRng(args.seed, 1e9 + j);
    const driver = new Driver(EditorState.create({ doc: note.source, extensions: extensionsFor(rng, true) }), shadow, 1.7e12);
    runDriver(`corpus ${note.name}`, `${script} ${args.cases} ${args.seed}`, rng, driver, 200);
    notes++;
  }
}

console.log(
  `shadow oracle: ${cases} random cases (${starts.fresh} fresh, ${starts.restored} restored, ${starts.late} late) + ${notes} corpus notes, ` +
    `${steps} transactions, ${fullChecks} full checks (${treeChecks} with a tree), seed ${args.seed}, ${failures} mismatches`,
);
if (cases === 0 || treeChecks === 0) {
  console.error("shadow oracle: nothing was checked");
  process.exit(1);
}
process.exit(failures === 0 ? 0 : 1);

/** Wraps every shadow `wasm` makes so a test can corrupt one call. */
function tampered(hooks: {
  apply?: (inner: WasmShadow, call: number, args: Parameters<WasmShadow["apply"]>) => WasmShadow;
  undo?: (inner: WasmShadow) => WasmShadow | undefined;
  tree?: (inner: WasmShadow) => Uint32Array;
}): ShadowWasm {
  let calls = 0;
  const wrap = (inner: WasmShadow): WasmShadow => ({
    apply: (...a) => wrap(hooks.apply ? hooks.apply(inner, ++calls, a) : inner.apply(...a)),
    undo: (t) => {
      const next = hooks.undo ? hooks.undo(inner) : inner.undo(t);
      return next && wrap(next);
    },
    redo: (t) => {
      const next = inner.redo(t);
      return next && wrap(next);
    },
    undoSelection: (t) => {
      const next = inner.undoSelection(t);
      return next && wrap(next);
    },
    redoSelection: (t) => {
      const next = inner.redoSelection(t);
      return next && wrap(next);
    },
    changedRanges: () => inner.changedRanges(),
    changesJson: () => inner.changesJson(),
    historyJson: () => inner.historyJson(),
    length: () => inner.length(),
    text: () => inner.text(),
    slice: (from, to) => inner.slice(from, to),
    selectionJson: () => inner.selectionJson(),
    undoDepth: () => inner.undoDepth(),
    redoDepth: () => inner.redoDepth(),
    tree: () => (hooks.tree ? hooks.tree(inner) : inner.tree()),
  });
  return { seed: (doc, sel, hist) => wrap(real.seed(doc, sel, hist)), nodeNames: real.nodeNames };
}

/** Corrupts the shadow four ways and requires exactly one report of the
 *  right kind each time, with a replay payload, and silence after it. */
function selfTests() {
  const tests: { name: string; expect: MismatchKind; wasm: ShadowWasm }[] = [
    {
      name: "a stray character",
      expect: "length",
      wasm: tampered({
        apply: (inner, call, a) => {
          const next = inner.apply(...a);
          return call === 3 ? next.apply(JSON.stringify([[0, "§"], next.length()]), null, null, false, null, a[5]) : next;
        },
      }),
    },
    {
      name: "an edit left out of the history",
      expect: "depth",
      wasm: tampered({
        apply: (inner, call, [c, s, e, add, i, t]) => inner.apply(c, s, e, call === 3 ? false : add, i, t),
      }),
    },
    { name: "an undo that finds nothing", expect: "undo", wasm: tampered({ undo: () => undefined }) },
    { name: "a node missing from the tree", expect: "tree", wasm: tampered({ tree: (inner) => inner.tree().slice(0, -3) }) },
  ];
  for (const test of tests) {
    const got: Report[] = [];
    const s = createEditorShadow({ wasm: () => test.wasm, reporter: collector(got) });
    const d = new Driver(EditorState.create({ doc: "# Title\n\nSome *text*.\n", extensions: [history(), language, s.extension] }), s, 1000);
    const type = (text: string) => {
      for (const ch of text) {
        d.time += 50;
        d.tr({ ...d.state.replaceSelection(ch), userEvent: "input.type" });
      }
    };
    type("ab");
    d.time += 2000;
    type("cd");
    d.time += 2000;
    type("ef");
    d.full();
    d.command(undo);
    d.full();
    type("more text");
    d.full();
    const label = `self-test "${test.name}"`;
    if (got.length !== 1 || got[0].kind !== test.expect) {
      fail(label, script, `expected one ${test.expect} report, got ${JSON.stringify(got.map((r) => r.kind))}`);
      continue;
    }
    const replay = (got[0].details as { replay?: { seed?: unknown; steps?: unknown[] } }).replay;
    if (!replay?.seed || !replay.steps?.length) fail(label, script, "the report carries no replay");
    if (s.status(d.state) !== "stopped") fail(label, script, `the shadow is ${s.status(d.state)} after its report`);
  }
}

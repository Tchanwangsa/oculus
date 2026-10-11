import { isolateHistory } from "@codemirror/commands";
import { ChangeSet, EditorSelection, EditorState, StateField, Transaction } from "@codemirror/state";

import { basisOf, plainSelection, poppedSelection, richSelection, seedOf } from "./basis";
import type { Context } from "./context";
import { firstDifference } from "./diff";
import type { MismatchKind, Pop, ShadowWasm, Step, WasmShadow } from "./types";
import { STOPPED, UNSEEDED, extend, isLive, type Live, type ShadowValue } from "./value";

/** The user events `@codemirror/commands`' history gives its own
 *  transactions. The shadow pops its own history for these instead of
 *  applying them, and checks the result against the transaction. */
function popOf(userEvent: string | undefined): Pop | null {
  switch (userEvent) {
    case "undo":
      return "undo";
    case "redo":
      return "redo";
    case "select.undo":
      return "undoSelection";
    case "select.redo":
      return "redoSelection";
    default:
      return null;
  }
}

function seeded(wasm: ShadowWasm, state: EditorState): Live {
  const seed = basisOf(state);
  const json = seedOf(seed);
  return {
    shadow: wasm.seed(json.doc, json.selection, json.history),
    chain: { stopped: false },
    names: wasm.nodeNames,
    seed,
    records: [],
  };
}

/** The new state's selection: one range unless the state allows more. */
function selectionAfter(tr: Transaction): EditorSelection {
  return tr.startState.facet(EditorState.allowMultipleSelections) ? tr.newSelection : tr.newSelection.asSingle();
}

type Problem = { kind: MismatchKind; details: Record<string, unknown> };

/** The checks every transaction gets: length, the text of the changed
 *  ranges, and the selection. */
function quickCheck(shadow: WasmShadow, tr: Transaction, selection: EditorSelection): Problem | null {
  if (shadow.length() !== tr.newDoc.length) {
    return { kind: "length", details: { expected: tr.newDoc.length, actual: shadow.length() } };
  }
  const ranges = shadow.changedRanges();
  for (let i = 0; i < ranges.length; i += 4) {
    const [from, to] = [ranges[i + 2], ranges[i + 3]];
    const want = tr.newDoc.sliceString(from, to);
    const got = shadow.slice(from, to);
    if (got !== want) return { kind: "text", details: { from, to, ...firstDifference(want, got) } };
  }
  const want = plainSelection(selection);
  const got = shadow.selectionJson();
  return got === want ? null : { kind: "selection", details: { expected: want, actual: got } };
}

/** `live` after `tr`, or `STOPPED` once it disagrees. */
function mirror(ctx: Context, live: Live, tr: Transaction): ShadowValue {
  const userEvent = tr.annotation(Transaction.userEvent);
  const time = tr.annotation(Transaction.time) ?? Date.now();
  const selection = selectionAfter(tr);
  const before = basisOf(tr.startState);
  const fail = (at: Live, kind: MismatchKind, details: Record<string, unknown>): ShadowValue => {
    ctx.mismatch(at, kind, { userEvent, ...details });
    return STOPPED;
  };
  const pop = popOf(userEvent);
  let next: Live;
  if (pop) {
    const popped = live.shadow[pop](time);
    const steps: Step[] = [{ op: pop, time }];
    const want = JSON.stringify(tr.changes.toJSON());
    next = extend(live, popped ?? live.shadow, { before, steps });
    if (!popped) return fail(next, "undo", { expected: want, actual: `${pop} found nothing to pop` });
    const got = popped.changesJson();
    if (got !== want) return fail(next, "undo", { expected: want, actual: got });
    if ((pop === "undoSelection" || pop === "redoSelection") && popped.selectionJson() !== plainSelection(selection)) {
      // A transaction filter (the maths field's) may move the selection
      // CodeMirror restored. When the shadow restored the one the history
      // held, follow the transaction outside the history.
      const held = poppedSelection(tr.startState, pop);
      if (held && popped.selectionJson() === plainSelection(held)) {
        const follow: Step = {
          op: "apply",
          changes: JSON.stringify(ChangeSet.empty(tr.newDoc.length).toJSON()),
          selection: richSelection(selection),
          userEvent: null,
          addToHistory: false,
          isolate: null,
          time,
        };
        steps.push(follow);
        next = { ...next, shadow: popped.apply(follow.changes, follow.selection, null, false, null, time) };
      }
    }
  } else {
    const step: Step = {
      op: "apply",
      changes: JSON.stringify(tr.changes.toJSON()),
      selection: tr.selection ? richSelection(selection) : null,
      userEvent: userEvent ?? null,
      addToHistory: tr.annotation(Transaction.addToHistory) !== false,
      isolate: tr.annotation(isolateHistory) ?? null,
      time,
    };
    const shadow = live.shadow.apply(step.changes, step.selection, step.userEvent, step.addToHistory, step.isolate, time);
    next = extend(live, shadow, { before, steps: [step] });
  }
  const problem = quickCheck(next.shadow, tr, selection);
  return problem ? fail(next, problem.kind, problem.details) : next;
}

/** The shadow of each state: seeded from the first state it sees once the
 *  wasm is ready, then mirroring every transaction. */
export function shadowField(ctx: Context): StateField<ShadowValue> {
  return StateField.define<ShadowValue>({
    create(state) {
      return ctx.guard<ShadowValue>(null, STOPPED, () => {
        const wasm = ctx.off ? null : ctx.wasm();
        return wasm ? seeded(wasm, state) : UNSEEDED;
      });
    },
    update(value, tr) {
      if (value === STOPPED || ctx.off) return STOPPED;
      if (isLive(value) && value.chain.stopped) return STOPPED;
      const live = isLive(value)
        ? value
        : ctx.guard<ShadowValue>(null, STOPPED, () => {
            const wasm = ctx.wasm();
            return wasm ? seeded(wasm, tr.startState) : UNSEEDED;
          });
      if (!isLive(live)) return live;
      return ctx.guard<ShadowValue>(live.chain, STOPPED, () => mirror(ctx, live, tr));
    },
  });
}

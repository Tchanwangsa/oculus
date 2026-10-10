import { isolateHistory } from "@codemirror/commands";
import type { ChangeSpec, EditorState, TransactionSpec } from "@codemirror/state";

import type { FieldChange, Step } from "@/lib/maths";
import { FIELD_INPUT, fieldWrite, writableSpan } from "../mathFieldEdits";
import { visualMath, type ActiveMath } from "../mathField/visual-state";

/**
 * How the Rust field's steps reach the note: the source the edit model
 * edits is the note's LaTeX between the delimiters, so each step's changes
 * are written as they are, moved to where that source starts in the note.
 * Only a block's edges are kept to one line break each once it spans
 * lines, as the note's `$$` lines need. Over the state and `dispatch`
 * alone, so the write path tests without a DOM.
 */

/** Where steps are written: an `EditorView`, or a state and a dispatch. */
export interface NoteView {
  readonly state: EditorState;
  dispatch(spec: TransactionSpec): void;
}

/** What the field knows of its maths in the note between steps. */
export interface FieldText {
  /** The maths this field edits (`ActiveMath.id`). */
  readonly id: number;
  /** The LaTeX (trimmed, as the widget sees it) last written or loaded, so
   *  the field's own writes don't echo back into it. */
  shown: string;
  /** The edit model's source the note holds: what a step's changes apply
   *  to (`shown` with the source's own edge whitespace). */
  written: string;
}

/** The maths `text` may write to, or null when it is gone or was changed
 *  from outside since the field last saw it. */
export function writableTarget(note: NoteView, text: FieldText): ActiveMath | null {
  return writableSpan(note.state, visualMath(note.state), text.id, text.shown);
}

/**
 * One step into the note: its changes as one transaction (an undo step of
 * its own when `isolate`), then a shortcut's rewrite as a second that always
 * is, so undo gives back what was typed before the expansion. The effect is
 * the caller's.
 */
export function writeStep(note: NoteView, text: FieldText, step: Step) {
  const before = text.written;
  const { field } = step;
  const head = field.stops[field.head];
  const rewrite = step.rewrite ?? [];
  const middle = applyChanges(before, step.changes);
  if (applyChanges(middle, rewrite) !== field.source) {
    // The step's parts don't add up to its field: write the result whole.
    write(note, text, before, [{ from: 0, to: before.length, insert: field.source }], head, step.isolate);
  } else {
    write(note, text, before, step.changes, rewrite.length ? endOfChanges(before, middle, step.changes) : head, step.isolate);
    write(note, text, middle, rewrite, head, true);
  }
  text.written = field.source;
}

function write(
  note: NoteView,
  text: FieldText,
  before: string,
  changes: readonly FieldChange[],
  head: number,
  isolate: boolean,
) {
  if (!changes.length) return;
  const target = writableTarget(note, text);
  const w = target && noteWrite(note.state, target, text.id, before, changes, head, isolate);
  if (!w) return;
  // Before the dispatch: the widget's `sync` during it must see its own write.
  text.shown = w.shown;
  note.dispatch(w.spec);
}

/** The note's LaTeX changed to `source` under the field (an undo): `text`
 *  takes it, and the caret goes to the end of what changed (`caretAfterEdit`;
 *  `head` is the field's caret in the source it had). Null when it is the
 *  field's own. */
export function resync(text: FieldText, source: string, head: number): number | null {
  if (source === text.shown) return null;
  const lead = text.written.length - text.written.trimStart().length;
  const caret = caretAfterEdit(text.shown, source, Math.max(0, head - lead));
  text.shown = source;
  text.written = source;
  return caret;
}

/** `changes` (sorted, in `source`'s offsets) applied to `source`. */
export function applyChanges(source: string, changes: readonly FieldChange[]): string {
  let out = source;
  for (let i = changes.length - 1; i >= 0; i--) {
    const { from, to, insert } = changes[i];
    out = out.slice(0, from) + insert + out.slice(to);
  }
  return out;
}

/** Where the last of `changes` ends once they are applied: nothing after
 *  its `to` moves, so it keeps its distance from the end. */
export function endOfChanges(before: string, after: string, changes: readonly FieldChange[]): number {
  const last = changes[changes.length - 1];
  return last ? after.length - (before.length - last.to) : after.length;
}

const leadOf = (s: string) => s.length - s.trimStart().length;

/** One write into the note and the maths' trimmed LaTeX after it (what the
 *  field then counts as its own, `shown`). */
export interface NoteWrite {
  spec: TransactionSpec;
  shown: string;
}

/**
 * The transaction writing `changes` (to the field's source `before`) into
 * `target`, the note's caret at `head` (an offset of the source after), an
 * undo step of its own when `isolate`. Null when nothing changes.
 */
export function noteWrite(
  state: EditorState,
  target: ActiveMath,
  id: number,
  before: string,
  changes: readonly FieldChange[],
  head: number,
  isolate: boolean,
): NoteWrite | null {
  const after = applyChanges(before, changes);
  if (after === before) return null;
  const annotations = isolate ? [fieldWrite.of(id), isolateHistory.of("full")] : fieldWrite.of(id);
  const current = state.sliceDoc(target.from, target.to);
  const caret = Math.max(0, Math.min(head, after.length));
  // An empty `\(\)` (a `$` typed at a line's start) becomes `$…$` once it
  // holds something; the caret stays inside, so the field stays open.
  if (!target.display && !current.trim() && after.trim() && state.sliceDoc(target.start, target.from) === "\\(") {
    return {
      spec: {
        changes: { from: target.start, to: target.end, insert: `$${after}$` },
        selection: { anchor: target.start + 1 + caret },
        userEvent: FIELD_INPUT,
        annotations,
      },
      shown: after.trim(),
    };
  }
  // The note's LaTeX is a lead, the field's source, then a trail.
  const pre = current.trim() ? leadOf(current) - leadOf(before) : current.length;
  const start = target.from + pre;
  const oldLead = current.slice(0, pre);
  const oldTrail = current.slice(pre + before.length);
  let lead = oldLead;
  let trail = oldTrail;
  // A block on lines of its own: its LaTeX starts and ends a line, one
  // break each, which the source's own edge breaks can be.
  if (target.block && (after.includes("\n") || !current.trim())) {
    lead = after.startsWith("\n") ? "" : "\n";
    trail = after.endsWith("\n") ? "" : "\n";
  }
  // Sorted, in the old document's offsets: CodeMirror keeps inserts at one
  // position in this order.
  const specs: ChangeSpec[] = [];
  if (lead !== oldLead) specs.push({ from: target.from, to: start, insert: lead });
  for (const c of changes) specs.push({ from: start + c.from, to: start + c.to, insert: c.insert });
  if (trail !== oldTrail) specs.push({ from: start + before.length, to: target.to, insert: trail });
  return {
    spec: {
      changes: specs,
      selection: { anchor: target.from + lead.length + caret },
      userEvent: FIELD_INPUT,
      annotations,
    },
    shown: (lead + after + trail).trim(),
  };
}

/**
 * Where the field's caret goes when the note's LaTeX changes under it (an
 * undo, a redo): the end of what changed, comparing `before` and `after`
 * from both ends. The common tail stops at the old caret, which places a
 * change that repeats the text beside it (as `caretAfterChange` does over
 * MathLive's atoms).
 */
export function caretAfterEdit(before: string, after: string, caret: number): number {
  const max = Math.min(before.length, after.length);
  const tail = Math.max(0, Math.min(max, before.length - caret));
  let s = 0;
  while (s < tail && before[before.length - 1 - s] === after[after.length - 1 - s]) s++;
  return after.length - s;
}

import { syntaxTree } from "@codemirror/language";
import { Facet, StateEffect, StateField, type EditorState, type Range, type RangeSet } from "@codemirror/state";
import { BlockWrapper, Decoration, EditorView, WidgetType, type DecorationSet } from "@codemirror/view";

import type { DiffChange, DiffHunk } from "@/lib/notes/versionDiff";

import { diffPlan } from "./diffPlan";

/**
 * The change marks over a shown version: inserted text tinted, deleted text
 * struck through where it was (between lines when it spans them), a bar
 * beside each changed line, and a widget construct a change falls in
 * tinted whole — inline ones through an outer mark, which wraps the widget,
 * block ones through a `BlockWrapper`.
 * A state field, since deleted lines are block widgets.
 */

/** What the shown text is compared against; null draws no marks. */
export interface VersionDiff {
  base: string;
  changes: readonly DiffChange[];
  hunks: readonly DiffHunk[];
}

export const versionDiff = Facet.define<VersionDiff | null, VersionDiff | null>({
  combine: (values) => values[0] ?? null,
});

/** The hunk navigation last moved to, or null. */
export const setCurrentHunk = StateEffect.define<number | null>();

class DeletedWidget extends WidgetType {
  constructor(
    readonly text: string,
    readonly block: boolean,
    /** ` cm-diff-current …` on the current hunk, else empty. */
    readonly lit: string,
  ) {
    super();
  }

  eq(other: DeletedWidget) {
    return other.text === this.text && other.block === this.block && other.lit === this.lit;
  }

  toDOM() {
    const dom = document.createElement(this.block ? "div" : "del");
    dom.className = (this.block ? "cm-diff-del-block" : "cm-diff-del") + this.lit;
    if (this.block) {
      const del = document.createElement("del");
      del.textContent = this.text;
      dom.appendChild(del);
    } else {
      dom.textContent = this.text;
    }
    return dom;
  }

  get estimatedHeight() {
    return this.block ? this.text.split("\n").length * 24 : -1;
  }
}

interface DiffDecorations {
  current: number | null;
  /** Counts moves, so landing on the same hunk again still flashes: the
   *  class alternates and the animation restarts. */
  flash: number;
  marks: DecorationSet;
  /** Marks over inline constructs: outer decorations wrap the widgets
   *  Live mode's replace decorations draw. */
  outer: DecorationSet;
  wrappers: RangeSet<BlockWrapper>;
}

function build(state: EditorState, current: number | null, flash: number): DiffDecorations {
  const diff = state.facet(versionDiff);
  const empty = { current, flash, marks: Decoration.none, outer: Decoration.none, wrappers: BlockWrapper.set([]) };
  if (!diff || !diff.changes.length) return empty;
  const plan = diffPlan(state, diff.base, diff.changes, diff.hunks);
  const lit = ` cm-diff-current cm-diff-flash-${flash % 2}`;
  const now = (hunk: number) => (hunk === current ? lit : "");
  const marks: Range<Decoration>[] = [];
  const outer: Range<Decoration>[] = [];
  const wrappers: Range<BlockWrapper>[] = [];
  for (const r of plan.inserts) marks.push(Decoration.mark({ class: `cm-diff-ins${now(r.hunk)}` }).range(r.from, r.to));
  for (const d of plan.deletions) {
    const widget = new DeletedWidget(d.text, d.block, now(d.hunk));
    marks.push(Decoration.widget({ widget, block: d.block, side: d.side }).range(d.at));
  }
  for (const k of plan.constructs) {
    const cls = `cm-diff-${k.kind}${now(k.hunk)}`;
    if (k.block) {
      wrappers.push(BlockWrapper.create({ tagName: "div", attributes: { class: `cm-diff-block ${cls}` } }).range(k.from, k.to));
    } else {
      // Inclusive, so it opens before and closes after the widget it wraps.
      outer.push(Decoration.mark({ class: cls, inclusive: true }).range(k.from, k.to));
    }
  }
  for (const l of plan.lines) {
    const cur = current != null && l.hunks.includes(current) ? " cm-diff-current-line" : "";
    marks.push(Decoration.line({ class: `cm-diff-line cm-diff-line-${l.kind}${cur}` }).range(l.at));
  }
  return {
    current,
    flash,
    marks: Decoration.set(marks, true),
    outer: Decoration.set(outer, true),
    wrappers: BlockWrapper.set(wrappers, true),
  };
}

export const diffField = StateField.define<DiffDecorations>({
  create: (state) => build(state, null, 0),
  update(value, tr) {
    let { current, flash } = value;
    for (const e of tr.effects) {
      if (e.is(setCurrentHunk)) {
        current = e.value;
        flash++;
      }
    }
    // The tree grows as the parser catches up; constructs come from it.
    const stale = flash !== value.flash || syntaxTree(tr.state) !== syntaxTree(tr.startState);
    return stale ? build(tr.state, current, flash) : value;
  },
  provide: (f) => [
    EditorView.decorations.from(f, (v) => v.marks),
    EditorView.outerDecorations.from(f, (v) => v.outer),
    EditorView.blockWrappers.from(f, (v) => v.wrappers),
  ],
});

/** The hunk navigation last moved to. */
export function currentHunk(state: EditorState): number | null {
  return state.field(diffField, false)?.current ?? null;
}

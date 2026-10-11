import type { EditorState, Range } from "@codemirror/state";
import { Decoration } from "@codemirror/view";
import type { SyntaxNode, SyntaxNodeRef } from "@lezer/common";

import { findRevealed } from "../../chrome/find";
import { fenceCode } from "../../syntax/codeLanguages";
import { focusedField } from "../../core/liveFocus";
import { ownsLines } from "../../math/mathContext";
import { MathFieldWidget, touchedMath, type ActiveMath } from "../../math/field/mathField";

/** Whether a selection range touches `from..to`, edges included. A match
 *  the find bar selected counts while the bar has focus (`find.ts`). */
export function touches(state: EditorState, from: number, to: number): boolean {
  if (!state.field(focusedField) && !findRevealed(state)) return false;
  return state.selection.ranges.some((r) => r.from <= to && r.to >= from);
}

/** The same, widened to whole lines. */
export function touchesLines(state: EditorState, from: number, to: number): boolean {
  return touches(state, state.doc.lineAt(from).from, state.doc.lineAt(to).to);
}

/** Whether a non-empty selection range covers part of `from..to`. */
export function selectedIn(state: EditorState, from: number, to: number): boolean {
  return state.selection.ranges.some((r) => !r.empty && r.from < to && r.to > from);
}

export const hide = Decoration.replace({});
export const mark = (cls: string) => Decoration.mark({ class: cls });
export const line = (cls: string) => Decoration.line({ class: cls });

/** `![alt](src)` pieces, or null for a reference image. */
export function imageParts(state: EditorState, node: SyntaxNode) {
  const url = node.getChild("URL");
  const marks = node.getChildren("LinkMark");
  if (!url || marks.length < 2) return null;
  return {
    src: state.sliceDoc(url.from, url.to),
    alt: state.sliceDoc(marks[0].to, marks[1].from),
  };
}

/** A picture with nothing else on its line, drawn as a block. */
export function isBlockImage(state: EditorState, node: SyntaxNodeRef): boolean {
  const ln = state.doc.lineAt(node.from);
  return node.to <= ln.to && ln.text.trim() === state.sliceDoc(node.from, node.to);
}

/** The maths between a node's two `MathMark`s, and where the caret goes. */
export function mathParts(state: EditorState, node: SyntaxNode) {
  const marks = node.getChildren("MathMark");
  if (marks.length < 2) return null;
  const from = marks[0].to;
  const to = marks[marks.length - 1].from;
  const source = state.sliceDoc(from, to);
  // Into the first content line, past a bare `$$` line.
  const newline = source.indexOf("\n");
  const caret = from - node.from + (newline >= 0 && source.slice(0, newline).trim() === "" ? newline + 1 : 0);
  return { source: source.trim(), caret, from, to };
}

/** Rendered or source, for maths the selection touches (`touchedMath`).
 *  A find match shows as source: the field, which would take it, needs focus. */
export function showsSource(state: EditorState, node: SyntaxNodeRef, parts: { from: number; to: number }, display: boolean) {
  if (!touches(state, node.from, node.to)) return false;
  return !state.field(focusedField) || touchedMath(state, node.from, parts.from, parts.to, display) === "source";
}

/** The visual field over the maths it is editing. */
export function fieldDecoration(state: EditorState, v: ActiveMath): Range<Decoration> {
  const widget = new MathFieldWidget(state.sliceDoc(v.from, v.to).trim(), v.display, v.block, v.id);
  return v.block
    ? Decoration.replace({ widget, block: true }).range(v.start, state.doc.lineAt(v.end).to)
    : Decoration.replace({ widget }).range(v.start, v.end);
}

/** A ```mermaid fence's source, or null for any other fence. */
export function mermaidCode(state: EditorState, node: SyntaxNode): string | null {
  const info = node.getChild("CodeInfo");
  if (!info || state.sliceDoc(info.from, info.to).split(/\s/)[0].toLowerCase() !== "mermaid") return null;
  return fenceCode(node, (a, b) => state.sliceDoc(a, b)).trimEnd();
}

/** A mermaid fence the block field draws (in place of its source, or under it
 *  while the selection touches it). */
export function isDrawnMermaid(state: EditorState, node: SyntaxNodeRef): boolean {
  return ownsLines(state, node) && mermaidCode(state, node.node) != null;
}

export interface Span {
  from: number;
  to: number;
}

/** The lone empty caret, or null. */
export function caretOf(state: EditorState): number | null {
  const { ranges, main } = state.selection;
  return ranges.length === 1 && main.empty ? main.head : null;
}

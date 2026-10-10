import { syntaxTree } from "@codemirror/language";
import { StateField, type EditorState, type Range } from "@codemirror/state";
import { Decoration, EditorView, type DecorationSet } from "@codemirror/view";

import { findRevealed } from "../../chrome/find";
import { noteHost } from "../../core/host";
import { setFocused } from "../../core/liveFocus";
import { ownsLines } from "../../math/mathContext";
import { visualMath, visualMathField } from "../../math/field/mathField";
import { TableWidget } from "../table";
import { parseTable } from "../tableModel";
import { ImageWidget, MathWidget, MermaidWidget, PropertiesWidget, RuleWidget } from "../widgets";
import {
  fieldDecoration,
  imageParts,
  isBlockImage,
  line,
  mathParts,
  mermaidCode,
  selectedIn,
  showsSource,
  touches,
  touchesLines,
} from "./shared";

function buildBlocks(state: EditorState): DecorationSet {
  const host = state.facet(noteHost);
  const out: Range<Decoration>[] = [];
  const visual = visualMath(state);
  if (visual?.block) out.push(fieldDecoration(state, visual));
  syntaxTree(state).iterate({
    enter: (node) => {
      switch (node.name) {
        case "BlockMath": {
          if (!ownsLines(state, node) || (visual?.block && node.from === visual.start)) return false;
          const parts = mathParts(state, node.node);
          if (!parts || showsSource(state, node, parts, true)) return false;
          const to = state.doc.lineAt(node.to).to;
          const widget = new MathWidget(parts.source, true, parts.caret, true, selectedIn(state, node.from, to));
          out.push(Decoration.replace({ widget, block: true }).range(node.from, to));
          return false;
        }
        case "HorizontalRule": {
          if (!ownsLines(state, node)) return false;
          // A rule needs a blank line above it, or it underlines a heading; the
          // blank lines beside it shrink to a gap unless the caret is on them.
          const first = state.doc.lineAt(node.from).number;
          const last = state.doc.lineAt(node.to).number;
          for (const n of [first - 1, last + 1]) {
            if (n < 1 || n > state.doc.lines) continue;
            const blank = state.doc.line(n);
            if (blank.text.trim() === "" && !touches(state, blank.from, blank.to)) {
              out.push(line("cm-hr-gap").range(blank.from));
            }
          }
          if (touchesLines(state, node.from, node.to)) return false;
          out.push(
            Decoration.replace({ widget: new RuleWidget(), block: true }).range(
              node.from,
              state.doc.lineAt(node.to).to,
            ),
          );
          return false;
        }
        case "Table": {
          // Never revealed: its cells are where it is edited (`tableKeys`).
          if (!ownsLines(state, node)) return false;
          const to = state.doc.lineAt(node.to).to;
          const source = state.sliceDoc(node.from, to);
          const layout = parseTable(source);
          if (layout) {
            out.push(Decoration.replace({ widget: new TableWidget(source, layout), block: true }).range(node.from, to));
          }
          return false;
        }
        case "Frontmatter": {
          const to = state.doc.lineAt(node.to).to;
          if (touches(state, node.from, to)) return false;
          out.push(
            Decoration.replace({ widget: new PropertiesWidget(state.sliceDoc(node.from, to)), block: true }).range(
              node.from,
              to,
            ),
          );
          return false;
        }
        case "Image": {
          if (!isBlockImage(state, node)) return false;
          const parts = imageParts(state, node.node);
          if (!parts) return false;
          const ln = state.doc.lineAt(node.from);
          const widget = new ImageWidget(host.imageSrc(parts.src), parts.alt, true, node.from - ln.from + 2);
          // While editing its source, the picture stays in view below it.
          out.push(
            touches(state, node.from, node.to)
              ? Decoration.widget({ widget, block: true, side: 1 }).range(ln.to)
              : Decoration.replace({ widget, block: true }).range(ln.from, ln.to),
          );
          return false;
        }
        case "FencedCode": {
          if (!ownsLines(state, node)) return false;
          const code = mermaidCode(state, node.node);
          if (code == null) return false;
          const first = state.doc.lineAt(node.from);
          const to = state.doc.lineAt(node.to).to;
          // A press puts the caret at the start of the first line inside.
          const caret = Math.min(first.to + 1, to) - node.from;
          // While editing its source, the diagram stays in view below it.
          out.push(
            touchesLines(state, node.from, node.to)
              ? Decoration.widget({ widget: new MermaidWidget(code, true, caret), block: true, side: 1 }).range(to)
              : Decoration.replace({ widget: new MermaidWidget(code, false, caret), block: true }).range(node.from, to),
          );
          return false;
        }
        // Nothing block-level lives inside these.
        case "Paragraph":
          return node.node.getChild("Image") != null;
        case "CodeBlock":
        case "ATXHeading1":
        case "ATXHeading2":
        case "ATXHeading3":
        case "ATXHeading4":
        case "ATXHeading5":
        case "ATXHeading6":
          return false;
      }
      return undefined;
    },
  });
  return Decoration.set(out, true);
}

export const blockField = StateField.define<DecorationSet>({
  create: buildBlocks,
  update(deco, tr) {
    const stale =
      tr.docChanged ||
      tr.selection ||
      tr.effects.some((e) => e.is(setFocused)) ||
      findRevealed(tr.state) !== findRevealed(tr.startState) ||
      tr.state.field(visualMathField) !== tr.startState.field(visualMathField, false) ||
      syntaxTree(tr.state) !== syntaxTree(tr.startState) ||
      tr.state.facet(noteHost) !== tr.startState.facet(noteHost);
    return stale ? buildBlocks(tr.state) : deco;
  },
  provide: (f) => EditorView.decorations.from(f),
});

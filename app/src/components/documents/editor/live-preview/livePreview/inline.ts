import { syntaxTree } from "@codemirror/language";
import type { Range } from "@codemirror/state";
import { Decoration, EditorView, ViewPlugin, type DecorationSet, type ViewUpdate } from "@codemirror/view";

import { findRevealed } from "../../chrome/find";
import { noteHost } from "../../core/host";
import { setFocused } from "../../core/liveFocus";
import { ownsLines } from "../../math/mathContext";
import { visualMath, visualMathField } from "../../math/field/mathField";
import { fenceCode, fenceLanguage } from "../../syntax/codeLanguages";
import { inlineCodeCitation } from "../../syntax/mentionSyntax";
import {
  BulletWidget,
  CheckboxWidget,
  CitationWidget,
  CodeHeaderWidget,
  ImageWidget,
  MathWidget,
} from "../widgets";
import { mathAtoms, mathIn } from "./math-blocks";
import {
  fieldDecoration,
  hide,
  imageParts,
  isBlockImage,
  isDrawnMermaid,
  line,
  mark,
  mathParts,
  showsSource,
  touches,
  touchesLines,
} from "./shared";

const HEADING = /^ATXHeading(\d)$/;
const SETEXT = /^SetextHeading(\d)$/;

const INLINE_STYLE: Record<string, { cls: string; mark: string }> = {
  Emphasis: { cls: "cm-em", mark: "EmphasisMark" },
  StrongEmphasis: { cls: "cm-strong", mark: "EmphasisMark" },
  Strikethrough: { cls: "cm-strike", mark: "StrikethroughMark" },
  InlineCode: { cls: "cm-inline-code", mark: "CodeMark" },
};

/** Parents that own their `URL`; any other `URL` is a bare GFM autolink. */
const LINKISH = new Set(["Link", "Image", "Autolink", "LinkReference"]);

function buildInline(view: EditorView): DecorationSet {
  const { state } = view;
  const host = state.facet(noteHost);
  const out: Range<Decoration>[] = [];
  const { from, to } = view.viewport;
  const visual = visualMath(state);
  if (visual && !visual.block) out.push(fieldDecoration(state, visual));

  /** Hide `[from, to)` plus one following space, as after `#` or `>`. */
  const hideWithSpace = (a: number, b: number) => {
    const end = state.sliceDoc(b, b + 1) === " " ? b + 1 : b;
    if (end > a) out.push(hide.range(a, end));
  };

  /** Line decorations for every line in `a..b`, first and last flagged. */
  const eachLine = (a: number, b: number, cls: string, ends = false) => {
    const first = state.doc.lineAt(a).number;
    const last = state.doc.lineAt(b).number;
    for (let n = first; n <= last; n++) {
      const ln = state.doc.line(n);
      if (ln.to < from || ln.from > to) continue;
      let c = cls;
      if (ends && n === first) c += ` ${cls}-first`;
      if (ends && n === last) c += ` ${cls}-last`;
      out.push(line(c).range(ln.from));
    }
  };

  syntaxTree(state).iterate({
    from,
    to,
    enter: (node) => {
      const name = node.name;

      const heading = HEADING.exec(name) ?? SETEXT.exec(name);
      if (heading) {
        out.push(line(`cm-h${heading[1]}`).range(state.doc.lineAt(node.from).from));
        if (HEADING.test(name) && !touchesLines(state, node.from, node.to)) {
          for (const m of node.node.getChildren("HeaderMark")) {
            // A closing run (`## Title ##`) takes the space before it instead.
            if (m.from === node.from) hideWithSpace(m.from, m.to);
            else {
              const before = state.sliceDoc(m.from - 1, m.from) === " " ? m.from - 1 : m.from;
              out.push(hide.range(before, m.to));
            }
          }
        }
        return undefined;
      }

      // A citation (what `@` writes) is a chip until the selection touches it.
      if (name === "InlineCode" && !touches(state, node.from, node.to)) {
        const cite = inlineCodeCitation(state.sliceDoc(node.from, node.to));
        if (cite) {
          out.push(Decoration.replace({ widget: new CitationWidget(cite) }).range(node.from, node.to));
          return false;
        }
      }

      const inline = INLINE_STYLE[name];
      if (inline) {
        out.push(mark(inline.cls).range(node.from, node.to));
        if (!touches(state, node.from, node.to)) {
          for (const m of node.node.getChildren(inline.mark)) out.push(hide.range(m.from, m.to));
        }
        return name === "InlineCode" ? false : undefined;
      }

      switch (name) {
        case "Link": {
          const url = node.node.getChild("URL");
          const marks = node.node.getChildren("LinkMark");
          if (!url || marks.length < 2 || marks[0].to >= marks[1].from) return undefined;
          out.push(mark("cm-link").range(marks[0].to, marks[1].from));
          if (!touches(state, node.from, node.to)) {
            out.push(hide.range(node.from, marks[0].to));
            out.push(hide.range(marks[1].from, node.to));
          }
          return undefined;
        }
        case "Autolink": {
          const url = node.node.getChild("URL");
          if (!url) return false;
          out.push(mark("cm-link").range(url.from, url.to));
          if (!touches(state, node.from, node.to)) {
            out.push(hide.range(node.from, url.from));
            out.push(hide.range(url.to, node.to));
          }
          return false;
        }
        case "URL": {
          if (!LINKISH.has(node.node.parent?.name ?? "")) {
            out.push(mark("cm-link").range(node.from, node.to));
          }
          return false;
        }
        case "Image": {
          if (isBlockImage(state, node) || touches(state, node.from, node.to)) return false;
          const parts = imageParts(state, node.node);
          if (!parts) return false;
          out.push(
            Decoration.replace({
              widget: new ImageWidget(host.imageSrc(parts.src), parts.alt, false, 2),
            }).range(node.from, node.to),
          );
          return false;
        }
        case "ListMark": {
          const item = node.node.parent;
          const list = item?.parent;
          if (list?.name === "OrderedList") {
            out.push(mark("cm-list-number").range(node.from, node.to));
            return false;
          }
          if (list?.name !== "BulletList" || touchesLines(state, node.from, node.to)) return false;
          // A task's checkbox stands in for its bullet.
          if (item?.getChild("Task")) hideWithSpace(node.from, node.to);
          else out.push(Decoration.replace({ widget: new BulletWidget() }).range(node.from, node.to));
          return false;
        }
        case "Task": {
          const marker = node.node.getChild("TaskMarker");
          if (!marker) return undefined;
          const checked = /x/i.test(state.sliceDoc(marker.from, marker.to));
          if (!touches(state, marker.from, marker.to)) {
            out.push(Decoration.replace({ widget: new CheckboxWidget(checked) }).range(marker.from, marker.to));
          }
          if (checked && marker.to < node.to) out.push(mark("cm-task-done").range(marker.to, node.to));
          return undefined;
        }
        case "Blockquote": {
          eachLine(node.from, node.to, "cm-quote");
          return undefined;
        }
        case "QuoteMark": {
          if (!touchesLines(state, node.from, node.to)) hideWithSpace(node.from, node.to);
          return false;
        }
        case "FencedCode": {
          if (isDrawnMermaid(state, node) && !touchesLines(state, node.from, node.to)) return false;
          eachLine(node.from, node.to, "cm-codeblock", true);
          const marks = node.node.getChildren("CodeMark");
          if (!marks.length || touchesLines(state, node.from, node.to)) {
            const info = node.node.getChild("CodeInfo");
            if (info) out.push(mark("cm-code-info").range(info.from, info.to));
            return false;
          }
          // The header takes the opening fence's place on its own line, so
          // revealing the fence moves nothing under the pointer.
          const read = (a: number, b: number) => state.sliceDoc(a, b);
          const lang = fenceLanguage(node.node, read);
          const head = state.doc.lineAt(marks[0].from);
          const header = new CodeHeaderWidget(lang?.label ?? "Plain text", lang?.auto ?? false, fenceCode(node.node, read));
          out.push(Decoration.replace({ widget: header }).range(marks[0].from, head.to));
          const close = marks.length > 1 ? marks[marks.length - 1] : null;
          if (close && close.from > head.to) {
            out.push(hide.range(close.from, close.to));
            out.push(line("cm-codeblock-close-hidden").range(state.doc.lineAt(close.from).from));
          }
          return false;
        }
        case "CodeBlock": {
          eachLine(node.from, node.to, "cm-codeblock", true);
          return false;
        }
        case "InlineMath": {
          if (visual && !visual.block && node.from === visual.start) return false;
          const parts = mathParts(state, node.node);
          if (parts && !showsSource(state, node, parts, false)) {
            out.push(
              Decoration.replace({ widget: new MathWidget(parts.source, false, parts.caret) }).range(
                node.from,
                node.to,
              ),
            );
          }
          return false;
        }
        case "BlockMath": {
          // Inside a quote or list item the field cannot draw it; one line
          // still renders inline, several stay source.
          if (ownsLines(state, node) || (visual && !visual.block && node.from === visual.start)) return false;
          if (state.doc.lineAt(node.from).number !== state.doc.lineAt(node.to).number) return false;
          const parts = mathParts(state, node.node);
          if (parts && !showsSource(state, node, parts, true)) {
            out.push(
              Decoration.replace({ widget: new MathWidget(parts.source, true, parts.caret) }).range(
                node.from,
                node.to,
              ),
            );
          }
          return false;
        }
      }
      return undefined;
    },
  });
  return Decoration.set(out, true);
}

export const inlinePlugin = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet;
    /** The rendered inline maths, atomic (`mathAtoms`). */
    atoms: DecorationSet;
    constructor(view: EditorView) {
      this.decorations = buildInline(view);
      this.atoms = mathIn(this.decorations, false);
    }
    update(u: ViewUpdate) {
      if (
        u.docChanged ||
        u.selectionSet ||
        u.viewportChanged ||
        u.focusChanged ||
        u.transactions.some((tr) => tr.effects.some((e) => e.is(setFocused))) ||
        findRevealed(u.state) !== findRevealed(u.startState) ||
        u.state.field(visualMathField) !== u.startState.field(visualMathField, false) ||
        syntaxTree(u.state) !== syntaxTree(u.startState) ||
        u.state.facet(noteHost) !== u.startState.facet(noteHost)
      ) {
        this.decorations = buildInline(u.view);
        this.atoms = mathIn(this.decorations, false);
      }
    }
  },
  {
    decorations: (v) => v.decorations,
    provide: (plugin) => EditorView.atomicRanges.of((view) => mathAtoms(view, view.plugin(plugin)?.atoms)),
  },
);

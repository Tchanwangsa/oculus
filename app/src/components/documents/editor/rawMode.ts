import { languageDataProp, syntaxHighlighting, syntaxTree } from "@codemirror/language";
import { Prec, type Extension, type Range } from "@codemirror/state";
import {
  Decoration,
  EditorView,
  ViewPlugin,
  gutters,
  highlightActiveLine,
  highlightActiveLineGutter,
  lineNumbers,
  type DecorationSet,
  type ViewUpdate,
} from "@codemirror/view";
import { tagHighlighter, tags } from "@lezer/highlight";

import { noteLanguage } from "./language";

/**
 * Raw mode: the note's source as a code editor shows it — monospace, line
 * numbers, markdown coloured with the `--color-syntax-*` tokens. Tags map to
 * `cm-raw-*` classes styled in `rawTheme`; scoped under the editor they beat
 * `noteHighlight`'s single-class rules on the same span, and their order there
 * settles a span with two (a heading's `#` is keyword, not operator). Fenced
 * code keeps `noteHighlight`'s nested-grammar colours. The parser has no YAML
 * or maths-source nodes, so `rawSpans` marks those itself.
 */

const muted = "var(--color-muted-foreground)";
const syntax = (name: string) => `var(--color-syntax-${name})`;
const tint = "color-mix(in srgb, var(--color-foreground) 4%, transparent)";

const rawTheme = EditorView.theme({
  ".cm-content, .cm-gutters": { fontFamily: "var(--font-mono)", fontSize: "13px", lineHeight: "1.6" },
  // Spans Live mode shrinks for prose keep the document's size here.
  ".cm-content .cm-code-text, .cm-content .cm-raw-code": { fontSize: "inherit" },

  // The page scrolls, not `.cm-scroller`, so the gutter is unfixed (see
  // `rawMode`) and drops the base z-index that would lift it over page chrome.
  "&.cm-editor .cm-gutters": {
    backgroundColor: "transparent",
    border: "none",
    color: muted,
    zIndex: "auto",
  },
  "&.cm-editor .cm-lineNumbers .cm-gutterElement": {
    minWidth: "28px",
    padding: "0 14px 0 0",
    textAlign: "right",
    fontVariantNumeric: "tabular-nums",
    color: `color-mix(in srgb, ${muted} 55%, transparent)`,
  },
  "&.cm-editor .cm-activeLine, &.cm-editor .cm-activeLineGutter": { backgroundColor: "transparent" },
  "&.cm-editor.cm-focused .cm-activeLine, &.cm-editor.cm-focused .cm-activeLineGutter": { backgroundColor: tint },
  "&.cm-editor.cm-focused .cm-lineNumbers .cm-activeLineGutter": { color: muted },

  // Later wins when a span carries two colours.
  ".cm-raw-meta": { color: syntax("meta") },
  ".cm-raw-number": { color: syntax("number") },
  ".cm-raw-type": { color: syntax("type") },
  ".cm-raw-comment": { color: syntax("comment") },
  ".cm-raw-function": { color: syntax("function") },
  ".cm-raw-property": { color: syntax("property") },
  ".cm-raw-operator": { color: syntax("operator") },
  ".cm-raw-keyword": { color: syntax("keyword") },
  ".cm-raw-string": { color: syntax("string") },
  ".cm-raw-bold": { fontWeight: "700" },
  ".cm-raw-italic": { fontStyle: "italic" },
});

/** Markdown tags → `cm-raw-*` classes, for the note grammar only. */
const rawHighlight = syntaxHighlighting(
  tagHighlighter(
    [
      {
        tag: [tags.heading1, tags.heading2, tags.heading3, tags.heading4, tags.heading5, tags.heading6],
        class: "cm-raw-keyword cm-raw-bold",
      },
      // Plain `heading` is a table's header row: bold, not a heading colour.
      { tag: tags.heading, class: "cm-raw-bold" },
      { tag: tags.strong, class: "cm-raw-bold" },
      { tag: tags.emphasis, class: "cm-raw-italic" },
      // Header, list, quote, emphasis, link, code, table and maths marks.
      { tag: tags.processingInstruction, class: "cm-raw-operator" },
      { tag: [tags.contentSeparator, tags.labelName, tags.escape, tags.atom], class: "cm-raw-meta" },
      { tag: tags.link, class: "cm-raw-function" },
      { tag: [tags.url, tags.string], class: "cm-raw-string" },
      { tag: tags.monospace, class: "cm-raw-string cm-raw-code" },
      { tag: [tags.quote, tags.comment], class: "cm-raw-comment" },
      { tag: tags.character, class: "cm-raw-number" },
      // Frontmatter and maths; `rawSpans` colours inside them.
      { tag: tags.special(tags.content), class: "cm-raw-code" },
    ],
    { scope: (type) => type.prop(languageDataProp) === noteLanguage.data },
  ),
);

// ── Frontmatter YAML and maths source ──────────────────────────────────────

const mark = (cls: string) => Decoration.mark({ class: `cm-raw-${cls}` });
const MARK = {
  property: mark("property"),
  operator: mark("operator"),
  string: mark("string"),
  number: mark("number"),
  comment: mark("comment"),
  type: mark("type"),
};
type Kind = keyof typeof MARK;
type Push = (from: number, to: number, kind: Kind) => void;

const YAML_KEY = /^("(?:[^"\\]|\\.)*"|'[^']*'|[^\s#'"[{\]},-][^#]*?|-[^\s#][^#]*?)\s*:(?=\s|$)/;
const NUMBERISH =
  /^(?:[-+]?(?:\d[\d_]*(?:\.\d*)?|\.\d+)(?:[eE][-+]?\d+)?|0x[\da-fA-F]+|0o[0-7]+|[-+]?\.inf|\.nan|true|false|yes|no|on|off|null|~)$/i;
const BLOCK_SCALAR = /^[|>][-+0-9]*/;

/** A scalar is a number (booleans and null too) or a string. */
function scalar(text: string, at: number, push: Push) {
  const lead = text.length - text.trimStart().length;
  const word = text.trim();
  if (word) push(at + lead, at + lead + word.length, NUMBERISH.test(word) ? "number" : "string");
}

/** `[a, b]` / `{k: v}`: punctuation as operators, the pieces as scalars.
 *  Returns where a trailing ` # comment` starts, or the text's end. */
function flow(text: string, at: number, push: Push): number {
  const end = /\s#/.exec(text)?.index ?? text.length;
  let piece = 0;
  for (let i = 0; i <= end; i++) {
    if (i < end && !"[]{},".includes(text[i])) continue;
    const part = text.slice(piece, i);
    const key = YAML_KEY.exec(part.trimStart());
    if (key) {
      const k = at + piece + part.length - part.trimStart().length;
      push(k, k + key[1].length, "property");
      push(k + key[0].length - 1, k + key[0].length, "operator");
      scalar(part.slice(part.length - part.trimStart().length + key[0].length), k + key[0].length, push);
    } else {
      scalar(part, at + piece, push);
    }
    if (i < end) push(at + i, at + i + 1, "operator");
    piece = i + 1;
  }
  return end;
}

/** A value after `key:` or `- `; returns whether it opens a block scalar. */
function value(text: string, at: number, push: Push): boolean {
  const lead = text.length - text.trimStart().length;
  const rest = text.slice(lead);
  const start = at + lead;
  if (!rest) return false;
  if (rest.startsWith("#")) {
    push(start, start + rest.length, "comment");
    return false;
  }
  let end = 0;
  let block = false;
  const quote = /^("(?:[^"\\]|\\.)*"?|'(?:[^']|'')*'?)/.exec(rest);
  const scalarMark = BLOCK_SCALAR.exec(rest);
  if (quote) {
    end = quote[0].length;
    push(start, start + end, "string");
  } else if (scalarMark) {
    end = scalarMark[0].length;
    push(start, start + end, "operator");
    block = true;
  } else if (rest[0] === "[" || rest[0] === "{") {
    end = flow(rest, start, push);
  } else {
    const hash = /\s#/.exec(rest);
    end = hash ? hash.index : rest.length;
    scalar(rest.slice(0, end), start, push);
  }
  const comment = rest.indexOf("#", end);
  if (comment >= 0 && /^\s*$/.test(rest.slice(end, comment))) push(start + comment, start + rest.length, "comment");
  return block;
}

/** Colours the YAML between the frontmatter fences, over `[from, to)`. */
function yaml(view: EditorView, bodyFrom: number, bodyTo: number, from: number, to: number, push: Push) {
  const doc = view.state.doc;
  // Lines indented past a `key: |` line are that scalar's text.
  let blockIndent = -1;
  for (let pos = bodyFrom; pos < bodyTo; ) {
    const line = doc.lineAt(pos);
    pos = line.to + 1;
    const text = line.text;
    const indent = text.length - text.trimStart().length;
    const visible = line.to >= from && line.from <= to;
    if (blockIndent >= 0 && (!text.trim() || indent > blockIndent)) {
      if (visible && text.trim()) push(line.from + indent, line.to, "string");
      continue;
    }
    blockIndent = -1;
    if (!text.trim()) continue;
    const local: Push = (f, t, kind) => {
      if (visible) push(f, t, kind);
    };
    if (text[indent] === "#") {
      local(line.from + indent, line.to, "comment");
      continue;
    }
    let i = indent;
    while (text[i] === "-" && (i + 1 === text.length || /\s/.test(text[i + 1]))) {
      local(line.from + i, line.from + i + 1, "operator");
      i++;
      while (/\s/.test(text[i] ?? "")) i++;
    }
    const key = YAML_KEY.exec(text.slice(i));
    if (key) {
      local(line.from + i, line.from + i + key[1].length, "property");
      const colon = i + key[0].length - 1;
      local(line.from + colon, line.from + colon + 1, "operator");
      i = colon + 1;
    }
    if (value(text.slice(i), line.from + i, local)) blockIndent = indent;
  }
}

function rawMarks(view: EditorView): DecorationSet {
  const out: Range<Decoration>[] = [];
  const push: Push = (from, to, kind) => {
    if (to > from) out.push(MARK[kind].range(from, to));
  };
  const { from, to } = view.viewport;
  syntaxTree(view.state).iterate({
    from,
    to,
    enter: (node) => {
      if (node.name === "Frontmatter") {
        const [open, close] = node.node.getChildren("FrontmatterMark");
        if (open && close) yaml(view, open.to + 1, close.from, from, to, push);
        return false;
      }
      if (node.name === "InlineMath" || node.name === "BlockMath") {
        const marks = node.node.getChildren("MathMark");
        const bodyTo = marks.length > 1 ? marks[marks.length - 1].from : node.to;
        if (marks.length) push(marks[0].to, bodyTo, "type");
        return false;
      }
      return node.name === "FencedCode" ? false : undefined;
    },
  });
  // Pushed in document order, except a line's comment after its value.
  return Decoration.set(out, true);
}

/** Frontmatter YAML and maths source. Highest precedence makes these the
 *  innermost spans, so they colour over the highlighter's. */
const rawSpans = Prec.highest(
  ViewPlugin.fromClass(
    class {
      decorations: DecorationSet;
      constructor(view: EditorView) {
        this.decorations = rawMarks(view);
      }
      update(u: ViewUpdate) {
        if (u.docChanged || u.viewportChanged || syntaxTree(u.state) !== syntaxTree(u.startState)) {
          this.decorations = rawMarks(u.view);
        }
      }
    },
    { decorations: (v) => v.decorations },
  ),
);

/** Raw mode's extension, swapped in by `modeExtension` in `extensions.ts`. */
export function rawMode(): Extension {
  return [
    gutters({ fixed: false }),
    lineNumbers(),
    highlightActiveLine(),
    highlightActiveLineGutter(),
    rawHighlight,
    rawSpans,
    rawTheme,
  ];
}

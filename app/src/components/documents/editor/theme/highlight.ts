import { HighlightStyle, languageDataProp, syntaxHighlighting } from "@codemirror/language";
import type { Extension } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { tags } from "@lezer/highlight";

import { noteLanguage } from "../core/language";
import { codeStyle } from "./codeStyle";
import { brand, mono, muted } from "./tokens";

/** Markdown source colouring: markers muted, inline code and LaTeX
 *  monospace. Scoped to the note grammar, so it never reaches nested code. */
const markdownHighlight = syntaxHighlighting(
  HighlightStyle.define(
    [
      // Strong before heading, so a heading's own weight wins over bold in it.
      { tag: tags.strong, fontWeight: "600" },
      { tag: tags.heading, fontWeight: "650" },
      { tag: tags.emphasis, fontStyle: "italic" },
      { tag: tags.strikethrough, textDecoration: "line-through" },
      { tag: tags.link, color: brand },
      { tag: tags.url, color: muted },
      { tag: tags.monospace, fontFamily: mono, fontSize: "0.92em" },
      { tag: tags.special(tags.content), fontFamily: mono, fontSize: "0.92em" },
      { tag: [tags.processingInstruction, tags.contentSeparator, tags.atom, tags.labelName], color: muted },
      { tag: tags.quote, color: muted },
    ],
    { scope: noteLanguage },
  ),
);

/** Fenced code's colouring, for every grammar except the note's own; also
 *  `CodeEditor`'s (`components/code/`). */
export const codeHighlight: Extension = [
  syntaxHighlighting({
    style: (t) => codeStyle.style(t),
    scope: (type) => type.prop(languageDataProp) !== noteLanguage.data,
  }),
  // A bare highlighter brings no CSS; this is the style's own module.
  EditorView.styleModule.of(codeStyle.module!),
];

export const noteHighlight: Extension = [markdownHighlight, codeHighlight];

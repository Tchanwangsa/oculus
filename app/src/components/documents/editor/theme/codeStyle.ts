import { HighlightStyle } from "@codemirror/language";
import { tags } from "@lezer/highlight";

/** Source colouring for every code surface: the note editor, `CodeEditor`
 *  and rendered markdown (`HighlightedCode`). */
export const codeStyle = HighlightStyle.define([
  { tag: [tags.keyword, tags.tagName, tags.deleted], color: "var(--color-syntax-keyword)" },
  {
    tag: [tags.string, tags.regexp, tags.character, tags.attributeValue, tags.inserted],
    color: "var(--color-syntax-string)",
  },
  { tag: tags.comment, color: "var(--color-syntax-comment)" },
  {
    tag: [tags.number, tags.bool, tags.null, tags.atom, tags.unit, tags.escape],
    color: "var(--color-syntax-number)",
  },
  {
    tag: [tags.function(tags.variableName), tags.function(tags.propertyName), tags.macroName],
    color: "var(--color-syntax-function)",
  },
  { tag: [tags.typeName, tags.className, tags.namespace], color: "var(--color-syntax-type)" },
  { tag: [tags.propertyName, tags.attributeName], color: "var(--color-syntax-property)" },
  { tag: tags.operator, color: "var(--color-syntax-operator)" },
  { tag: [tags.meta, tags.processingInstruction, tags.annotation], color: "var(--color-syntax-meta)" },
  { tag: tags.heading, fontWeight: "600" },
  { tag: tags.emphasis, fontStyle: "italic" },
  { tag: tags.strong, fontWeight: "600" },
]);

import { Language, LanguageSupport } from "@codemirror/language";
import { commonmarkLanguage, pasteURLAsLink } from "@codemirror/lang-markdown";
import { Autolink, Strikethrough, Table, TaskList, type MarkdownParser } from "@lezer/markdown";

import { CodeLanguages, codeTextFont } from "./codeLanguages";
import { FrontmatterSyntax } from "./frontmatter";
import { MathSyntax } from "./mathSyntax";

/**
 * The note grammar: CommonMark, the GFM pieces a note uses (tables,
 * strikethrough, task lists, bare-URL autolinks), maths, YAML frontmatter and
 * fenced code parsed in its own language (`codeLanguages.ts`).
 *
 * Built from `commonmarkLanguage` rather than `markdown()`, whose HTML support
 * would parse inline HTML in a note as HTML, CSS and JavaScript. It shares that
 * language's data facet, so the markdown keymap's commands still see it as
 * markdown.
 */
export const noteLanguage = new Language(
  commonmarkLanguage.data,
  (commonmarkLanguage.parser as MarkdownParser).configure([
    Table,
    Strikethrough,
    TaskList,
    Autolink,
    MathSyntax,
    FrontmatterSyntax,
    CodeLanguages,
  ]),
  [],
  "markdown",
);

export function noteMarkdown(): LanguageSupport {
  return new LanguageSupport(noteLanguage, [pasteURLAsLink, codeTextFont]);
}

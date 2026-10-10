import { Language, LanguageSupport } from "@codemirror/language";
import type { Input, PartialParse, TreeFragment } from "@lezer/common";
import { commonmarkLanguage, pasteURLAsLink } from "@codemirror/lang-markdown";
import { Autolink, Strikethrough, Table, TaskList, type MarkdownParser } from "@lezer/markdown";

import { CodeLanguages, codeTextFont } from "../syntax/codeLanguages";
import { FrontmatterSyntax, frontmatterFragments } from "../syntax/frontmatter";
import { MathSyntax } from "../math/mathSyntax";

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
const noteParser = (commonmarkLanguage.parser as MarkdownParser).configure([
  Table,
  Strikethrough,
  TaskList,
  Autolink,
  MathSyntax,
  FrontmatterSyntax,
  CodeLanguages,
]);

// Frontmatter's lookahead invalidates fragments a markdown config cannot
// filter, so the filter wraps this one parser; it stays a `MarkdownParser`.
const createParse = noteParser.createParse.bind(noteParser);
noteParser.createParse = (input: Input, fragments: readonly TreeFragment[], ranges): PartialParse =>
  createParse(input, frontmatterFragments(input, fragments), ranges);

export const noteLanguage = new Language(commonmarkLanguage.data, noteParser, [], "markdown");

export function noteMarkdown(): LanguageSupport {
  return new LanguageSupport(noteLanguage, [pasteURLAsLink, codeTextFont]);
}

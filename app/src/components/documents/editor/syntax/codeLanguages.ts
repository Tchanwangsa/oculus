import { LanguageDescription, ParseContext, syntaxTree } from "@codemirror/language";
import { languages } from "@codemirror/language-data";
import type { Range } from "@codemirror/state";
import { Decoration, ViewPlugin, type DecorationSet, type EditorView, type ViewUpdate } from "@codemirror/view";
import { parseMixed, type Input, type SyntaxNode } from "@lezer/common";
import { styleTags } from "@lezer/highlight";
import type { MarkdownConfig } from "@lezer/markdown";
import hljs from "highlight.js/lib/core";
import bash from "highlight.js/lib/languages/bash";
import cpp from "highlight.js/lib/languages/cpp";
import css from "highlight.js/lib/languages/css";
import go from "highlight.js/lib/languages/go";
import haskell from "highlight.js/lib/languages/haskell";
import java from "highlight.js/lib/languages/java";
import javascript from "highlight.js/lib/languages/javascript";
import json from "highlight.js/lib/languages/json";
import python from "highlight.js/lib/languages/python";
import r from "highlight.js/lib/languages/r";
import rust from "highlight.js/lib/languages/rust";
import sql from "highlight.js/lib/languages/sql";
import typescript from "highlight.js/lib/languages/typescript";
import xml from "highlight.js/lib/languages/xml";
import yaml from "highlight.js/lib/languages/yaml";

/**
 * The language inside a fenced code block. The fence's first info word names
 * a language-data grammar, loaded by dynamic import the first time a note
 * uses it. An untagged fence is guessed with highlight.js over a small subset
 * and stays plain when the guess is weak. `fenceLanguage` serves both the
 * parser and Live mode's header, so the label always names what is coloured.
 */

export interface CodeLanguage {
  /** The grammar, or null for a tag language-data does not know. */
  desc: LanguageDescription | null;
  label: string;
  /** Guessed from the code rather than named by the fence. */
  auto: boolean;
}

/** Tags that mean "no grammar": plain text, and fences other surfaces draw. */
const PLAIN = new Set(["text", "plain", "plaintext", "txt", "mermaid", "math"]);

/** Short tags language-data has no alias for. */
const ALIAS: Record<string, string> = {
  py: "python",
  rs: "rust",
  hs: "haskell",
  kt: "kotlin",
  md: "markdown",
  golang: "go",
  matlab: "octave",
};

/** Labels that differ from the grammar's name. */
const LABEL: Record<string, string> = { matlab: "MATLAB" };

const byTag = new Map<string, CodeLanguage | null>();

function fromTag(tag: string, auto: boolean): CodeLanguage | null {
  const key = (auto ? "a:" : "t:") + tag;
  const hit = byTag.get(key);
  if (hit !== undefined) return hit;
  const word = tag.toLowerCase();
  let lang: CodeLanguage | null = null;
  if (!PLAIN.has(word)) {
    const desc = LanguageDescription.matchLanguageName(languages, ALIAS[word] ?? word, true);
    lang = { desc, label: LABEL[word] ?? desc?.name ?? tag, auto };
  } else if (word === "mermaid" || word === "math") {
    lang = { desc: null, label: tag, auto };
  }
  byTag.set(key, lang);
  return lang;
}
/**
 * hljs id → the fence tag it stands for, and a pattern the code must also
 * match. hljs ranks by keyword hits, which short snippets and prose game (Java
 * scores as TypeScript, pseudocode as CSS); the pattern vetoes those guesses.
 */
const GUESS: Record<string, { tag: string; sign: RegExp }> = {
  python: {
    tag: "python",
    sign: /^\s*(def|class|import|from|elif|with|print)\b|\b(self|None|True|False|range|len)\b|:\s*$/m,
  },
  javascript: {
    tag: "javascript",
    sign: /\b(const|let|var|function|async|await|console|document|require)\b|=>|===/,
  },
  typescript: {
    tag: "typescript",
    sign: /\b(interface|type)\s+\w+|\w\??:\s*(string|number|boolean|any|void|unknown|never)\b/,
  },
  java: {
    tag: "java",
    sign: /\b(public|private|protected)\s+[\w<>[\], ]+\s+\w+\s*[(=;{]|\bSystem\.\w+\.|\bimport\s+java\.|@Override\b/,
  },
  // C and C++ share one grammar; `CPP_ONLY` decides the label.
  cpp: {
    tag: "c",
    sign: /#include\b|\b(int|void|char|float|double|long|unsigned|struct\s+\w+)\s*\**\s*\w+\s*[()[\]=;,]|\b(printf|malloc|sizeof|NULL|std::)/,
  },
  rust: { tag: "rust", sign: /\b(fn|impl|pub|mut|use\s+\w+::)\b|println!/ },
  go: { tag: "go", sign: /^\s*(package|func)\b|:=|\bfmt\./m },
  sql: {
    tag: "sql",
    sign: /^\s*(SELECT|INSERT|UPDATE|DELETE|CREATE|ALTER|DROP|WITH)\b/im,
  },
  bash: {
    tag: "bash",
    sign: /^\s*(#!|\$ |cd|ls|echo|export|sudo|git|mkdir|rm|cp|mv|cat|grep|curl|pip3?|bun|npm|make|chmod|source)\b|(^|[^\w$])\$\{?[A-Za-z_]\w*|\bif\s+\[|\bdone\b/m,
  },
  json: { tag: "json", sign: /^\s*[[{][\s\S]*"\s*:/ },
  xml: { tag: "xml", sign: /<\/?[A-Za-z][\w:.-]*(\s[^<>]*)?>/ },
  css: { tag: "css", sign: /[\w\-.#:\])*]\s*\{[^{}]*[\w-]+\s*:[^;{}]+;/ },
  haskell: {
    tag: "haskell",
    sign: /^\s*\w+\s*::\s*\S|^\s*(module|data|import)\s+[A-Z]|\bwhere\s*$|\\\w+\s*->/m,
  },
  r: { tag: "r", sign: /\b(library|c|data\.frame|read\.csv|ggplot|summary)\(|\w\$\w/ },
  yaml: { tag: "yaml", sign: /^\s*[\w-]+:\s*$[\s\S]*^\s+(- |[\w-]+:)/m },
};

const GRAMMARS = { bash, cpp, css, go, haskell, java, javascript, json, python, r, rust, sql, typescript, xml, yaml };
for (const [id, grammar] of Object.entries(GRAMMARS)) hljs.registerLanguage(id, grammar);
const IDS = Object.keys(GUESS);

/** hljs relevance a guess must reach, tuned on short note snippets. */
const MIN_RELEVANCE = 5;
/** Shorter code is too little to tell. */
const MIN_CHARS = 12;
const SAMPLE_CHARS = 2000;

/** C++ only: what turns the C/C++ guess into a C++ label. */
const CPP_ONLY = /\bstd::|\bnamespace\b|\btemplate\s*<|\bclass\s+\w+|\b(cout|cin|endl)\b|\b(public|private):|#include\s*<(iostream|vector|string|map|memory|algorithm)>/;

/** The fence tag for `sample`, or null when no guess is confident. */
function guessTag(sample: string): string | null {
  if (sample.replace(/\s/g, "").length < MIN_CHARS) return null;
  const ranked = IDS.filter((id) => GUESS[id].sign.test(sample))
    .map((id) => ({ id, relevance: hljs.highlight(sample, { language: id }).relevance }))
    .filter((c) => c.relevance >= MIN_RELEVANCE)
    .sort((a, b) => b.relevance - a.relevance);
  for (const { id } of ranked) {
    return id === "cpp" && CPP_ONLY.test(sample) ? "c++" : GUESS[id].tag;
  }
  return null;
}

/** Guesses by sampled code; the parser asks again on every reparse. */
const guesses = new Map<string, string | null>();
const GUESS_CACHE_MAX = 200;

function guess(code: string): string | null {
  const sample = code.slice(0, SAMPLE_CHARS);
  const hit = guesses.get(sample);
  if (hit !== undefined) {
    // Most recently used goes last, so eviction takes the oldest.
    guesses.delete(sample);
    guesses.set(sample, hit);
    return hit;
  }
  const tag = guessTag(sample);
  if (guesses.size >= GUESS_CACHE_MAX) guesses.delete(guesses.keys().next().value!);
  guesses.set(sample, tag);
  return tag;
}

/** The language for a fence's info string and code, or null for plain. */
export function codeLanguage(info: string, code: string): CodeLanguage | null {
  const tag = /\S*/.exec(info.trim())![0];
  if (tag) return fromTag(tag, false);
  const guessed = guess(code);
  return guessed ? fromTag(guessed, true) : null;
}

/** A `FencedCode` node's language, its text read through `read`. */
export function fenceLanguage(node: SyntaxNode, read: (from: number, to: number) => string): CodeLanguage | null {
  const info = node.getChild("CodeInfo");
  return codeLanguage(info ? read(info.from, info.to) : "", fenceCode(node, read));
}

/** The code between a fence's markers, without quote or list markers. */
export function fenceCode(node: SyntaxNode, read: (from: number, to: number) => string): string {
  return node
    .getChildren("CodeText")
    .map((t) => read(t.from, t.to))
    .join("");
}

/** Parses each fenced block's code with its language's grammar. */
export const CodeLanguages: MarkdownConfig = {
  // Highlight classes stop at a nested grammar's edge, so block code takes
  // its font from `codeTextFont` instead — tagged or not, the same.
  props: [styleTags({ CodeText: [] })],
  wrap: parseMixed((ref, input: Input) => {
    if (ref.name !== "FencedCode") return null;
    const desc = fenceLanguage(ref.node, (from, to) => input.read(from, to))?.desc;
    if (!desc) return null;
    // Until the grammar's import lands the block stays plain; the parse
    // reruns when it does.
    const parser = desc.support ? desc.support.language.parser : ParseContext.getSkippingParser(desc.load());
    return { parser, overlay: (n) => n.name === "CodeText", bracketed: true };
  }),
};

const codeText = Decoration.mark({ class: "cm-code-text" });

function codeTextMarks(view: EditorView): DecorationSet {
  const out: Range<Decoration>[] = [];
  const { from, to } = view.viewport;
  syntaxTree(view.state).iterate({
    from,
    to,
    enter: (node) => {
      if (node.name !== "CodeText") return undefined;
      if (node.to > node.from) out.push(codeText.range(node.from, node.to));
      return false;
    },
  });
  return Decoration.set(out);
}

/** Block code's monospace font, in Live and Raw alike. */
export const codeTextFont = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet;
    constructor(view: EditorView) {
      this.decorations = codeTextMarks(view);
    }
    update(u: ViewUpdate) {
      if (u.docChanged || u.viewportChanged || syntaxTree(u.state) !== syntaxTree(u.startState)) {
        this.decorations = codeTextMarks(u.view);
      }
    }
  },
  { decorations: (v) => v.decorations },
);

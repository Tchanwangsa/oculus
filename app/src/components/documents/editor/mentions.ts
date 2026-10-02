import type { Completion, CompletionContext, CompletionResult } from "@codemirror/autocomplete";
import { syntaxTree } from "@codemirror/language";
import type { SyntaxNode } from "@lezer/common";
import { createElement } from "react";
import { flushSync } from "react-dom";
import { createRoot } from "react-dom/client";
import type { Icon } from "@phosphor-icons/react";

import { searchNoteLinkFiles, type DbFile } from "@/lib/db";
import { categoryIconFor } from "@/lib/fileTypes";
import { libraryLinkHref } from "@/lib/libraryLinks";
import { fileTitle } from "@/lib/openFile";

import { noteHost } from "./host";
import { mathAt } from "./mathContext";

/**
 * `@` in a note links a library file: the query after it searches the note's
 * subject (`searchNoteLinkFiles`), and accepting writes `[title](../path)`
 * relative to the note, which ⌘-click and `FileViewer` both resolve
 * (`libraryLinkTarget`). Never in maths, where `@a` is a shorthand.
 */

/** The chat's caps (`useMentionMenu.ts`), so a stray `@` in prose stops
 *  matching as the sentence runs on. */
const MAX_MENTION = 60;
const MAX_MENTION_WORDS = 4;

const MENTION = new RegExp(`(?:^|\\s)@([^\\s@\`][^@\`]{0,${MAX_MENTION - 1}})?$`);

/** Where a link would be literal text, or already is markup. */
const NO_MENTION = new Set(["InlineCode", "FencedCode", "CodeBlock", "Frontmatter", "URL", "Autolink", "HTMLBlock", "CommentBlock"]);

function inCode(cx: CompletionContext): boolean {
  for (const side of [-1, 1] as const) {
    for (let n: SyntaxNode | null = syntaxTree(cx.state).resolveInner(cx.pos, side); n; n = n.parent) {
      if (NO_MENTION.has(n.name)) return true;
    }
  }
  return false;
}

/** The file behind each option, for `mentionOptionIcon`. */
const optionFiles = new WeakMap<Completion, DbFile>();

/** A file's title as link text: no extension, brackets escaped. */
function linkText(file: DbFile): string {
  let title = fileTitle(file);
  const ext = /\.[^./]+$/.exec(file.filename)?.[0];
  if (ext && title.toLowerCase().endsWith(ext.toLowerCase())) title = title.slice(0, -ext.length);
  return title.replace(/[\\[\]]/g, "\\$&");
}

/** The folder under the subject (`files/Week 1`), else the category. */
function folderLabel(file: DbFile): string {
  const rel = file.relative_path.replace(/^courses\/[^/]+\//, "");
  const dir = rel.includes("/") ? rel.slice(0, rel.lastIndexOf("/")) : "";
  return dir || file.category || "";
}

export async function mentionCompletionSource(cx: CompletionContext): Promise<CompletionResult | null> {
  const line = cx.state.doc.lineAt(cx.pos);
  const m = MENTION.exec(line.text.slice(0, cx.pos - line.from));
  if (!m) return null;
  const query = m[1] ?? "";
  if (query.split(/\s+/).filter(Boolean).length > MAX_MENTION_WORDS) return null;
  if (mathAt(cx.state, cx.pos) || inCode(cx)) return null;
  const { subjectId, notePath } = cx.state.facet(noteHost);
  if (subjectId == null || !notePath) return null;

  const files = await searchNoteLinkFiles(subjectId, notePath, query).catch(() => []);
  if (cx.aborted) return null;
  const options: Completion[] = [];
  for (const file of files) {
    const href = libraryLinkHref(notePath, file.relative_path);
    if (!href) continue;
    const option: Completion = {
      label: fileTitle(file),
      detail: folderLabel(file),
      apply: `[${linkText(file)}](${href})`,
    };
    optionFiles.set(option, file);
    options.push(option);
  }
  if (!options.length) return null;
  // The database already matched and ranked; CodeMirror keeps that order.
  return { from: cx.pos - query.length - 1, options, filter: false };
}

/** `optionClass`: mention rows take the sans label (maths rows are mono). */
export function mentionOptionClass(completion: Completion): string {
  return optionFiles.has(completion) ? "cm-mention-option" : "";
}

/** Phosphor icons are React components; each one's SVG is rendered once and
 *  reused as markup. Empty (and not cached) if React is mid-render. */
const iconMarkup = new Map<Icon, string>();

function iconHtml(icon: Icon): string {
  const cached = iconMarkup.get(icon);
  if (cached != null) return cached;
  const host = document.createElement("span");
  const root = createRoot(host);
  try {
    flushSync(() => root.render(createElement(icon, { size: 14 })));
  } catch {
    // Called inside a React render: no icon this time.
  }
  const html = host.innerHTML;
  root.unmount();
  if (html) iconMarkup.set(icon, html);
  return html;
}

/** `addToOptions` entry: the file-type glyph left of a mention's title. */
export const mentionOptionIcon = {
  position: 20,
  render(completion: Completion): Node | null {
    const file = optionFiles.get(completion);
    if (!file) return null;
    const dom = document.createElement("span");
    dom.className = "cm-mention-icon";
    dom.innerHTML = iconHtml(categoryIconFor(file));
    return dom;
  },
};

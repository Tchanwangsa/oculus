import type { Completion, CompletionContext, CompletionResult } from "@codemirror/autocomplete";
import { createElement } from "react";
import { flushSync } from "react-dom";
import { createRoot } from "react-dom/client";
import type { Icon } from "@phosphor-icons/react";

import { searchNoteLinkFiles, type NoteLinkFile } from "@/lib/db";
import { categoryIconFor } from "@/lib/fileTypes";
import { displayCode } from "@/lib/format";
import { fileTitle } from "@/lib/openFile";

import { noteHost } from "./host";
import { mathAt } from "./mathContext";
import { mentionText } from "./mentionSyntax";
import { ancestorAt } from "./syntax";

/**
 * `@` in a note mentions a library file: the query after it searches the
 * host's subject, or the whole library without one (`searchNoteLinkFiles`),
 * and accepting writes the backticked library path the chat composer sends,
 * which Live mode draws as a chip (`mentionSyntax.ts`). Never in maths, where
 * `@a` is a shorthand.
 */

/** The chat's caps (`useMentionMenu.ts`), so a stray `@` in prose stops
 *  matching as the sentence runs on. */
const MAX_MENTION = 60;
const MAX_MENTION_WORDS = 4;

const MENTION = new RegExp(`(?:^|\\s)@([^\\s@\`][^@\`]{0,${MAX_MENTION - 1}})?$`);

/** Where a link would be literal text, or already is markup. */
const NO_MENTION = new Set(["InlineCode", "FencedCode", "CodeBlock", "Frontmatter", "URL", "Autolink", "HTMLBlock", "CommentBlock"]);

/** The file behind each option, for `mentionOptionIcon`. */
const optionFiles = new WeakMap<Completion, NoteLinkFile>();

/** The folder under the subject (`files/Week 1`), else the category. */
function folderLabel(file: NoteLinkFile): string {
  const rel = file.relative_path.replace(/^courses\/[^/]+\//, "");
  const dir = rel.includes("/") ? rel.slice(0, rel.lastIndexOf("/")) : "";
  return dir || file.category || "";
}

/** One menu row; across the whole library its detail leads with the subject,
 *  as the chat's `@` menu does. */
function mentionOption(file: NoteLinkFile, wholeLibrary: boolean): Completion {
  const folder = folderLabel(file);
  const option: Completion = {
    label: fileTitle(file),
    detail: wholeLibrary ? [displayCode(file.subject_code), folder].filter(Boolean).join(" · ") : folder,
    apply: mentionText(file.relative_path),
  };
  optionFiles.set(option, file);
  return option;
}

export async function mentionCompletionSource(cx: CompletionContext): Promise<CompletionResult | null> {
  const line = cx.state.doc.lineAt(cx.pos);
  const m = MENTION.exec(line.text.slice(0, cx.pos - line.from));
  if (!m) return null;
  const query = m[1] ?? "";
  if (query.split(/\s+/).filter(Boolean).length > MAX_MENTION_WORDS) return null;
  if (mathAt(cx.state, cx.pos) || ancestorAt(cx.state, cx.pos, (node) => NO_MENTION.has(node.name), [-1, 1])) return null;
  const { subjectId, notePath } = cx.state.facet(noteHost);

  const files = await searchNoteLinkFiles(subjectId, notePath, query).catch(() => []);
  if (cx.aborted) return null;
  const options = files.map((file) => mentionOption(file, subjectId == null));
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

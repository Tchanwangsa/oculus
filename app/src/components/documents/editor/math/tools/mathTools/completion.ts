import {
  snippetCompletion,
  type Completion,
  type CompletionContext,
  type CompletionResult,
} from "@codemirror/autocomplete";
import { EditorView } from "@codemirror/view";

import { mathAt } from "../../mathContext";
import { COMMON_COMMANDS, MATH_COMMANDS, MATH_TABS, previewOf, snippetTemplate } from "../mathPalette";
import { commandOf, recordCommand, recordUse } from "../mathUsage";
import { el, previewHtml } from "./dom";
import { sourceOf, subjectOf } from "./insert";

/** Where the last accepted completion left the caret, so the character
 *  typed after a field-less one (`\alpha` then a space) doesn't count twice. */
const completedAt = new WeakMap<EditorView, number>();

/** A `\command` typed out in maths counts as used once a character that
 *  can't continue its name follows it. */
export const typedCommands = EditorView.inputHandler.of((view, from, to, text) => {
  if (from !== to || text.length !== 1 || /[a-zA-Z]/.test(text)) return false;
  const done = completedAt.get(view) === from;
  completedAt.delete(view);
  if (done || !mathAt(view.state, from)) return false;
  const name = /(?<!\\)\\[a-zA-Z]+$/.exec(view.state.sliceDoc(Math.max(0, from - 32), from))?.[0];
  if (name) recordCommand(name, subjectOf(view));
  return false;
});

/** Completion rows' KaTeX previews, by option. */
const optionPreviews = new WeakMap<Completion, string>();
let options: Completion[] | null = null;

/** Every palette entry and extra command led by a `\command`, once each. */
function mathOptions(): Completion[] {
  if (options) return options;
  const seen = new Set<string>();
  options = [];
  for (const entry of [...MATH_TABS.flatMap((t) => t.entries), ...MATH_COMMANDS]) {
    const name = commandOf(entry.template);
    if (!name || seen.has(entry.template)) continue;
    seen.add(entry.template);
    const rest = sourceOf(entry.template).slice(name.length).trim();
    const option = snippetCompletion(snippetTemplate(entry.template), {
      label: name,
      detail: rest.length > 18 ? `${rest.slice(0, 17)}…` : rest || undefined,
      boost: COMMON_COMMANDS.has(name) ? 1 : 0,
    });
    const insert = option.apply as (view: EditorView, c: Completion, from: number, to: number) => void;
    option.apply = (view, c, from, to) => {
      insert(view, c, from, to);
      completedAt.set(view, view.state.selection.main.head);
      recordUse(entry, subjectOf(view));
    };
    optionPreviews.set(option, previewOf(entry));
    options.push(option);
  }
  return options;
}

/**
 * Answers only inside maths (`mathAt`): after `\` and a letter while typing,
 * or anywhere on Ctrl-Space. CodeMirror's fuzzy match ranks prefix matches
 * first. Exported so the editor's one `autocompletion()` can hold other sources.
 */
export function mathCompletionSource(cx: CompletionContext): CompletionResult | null {
  const math = mathAt(cx.state, cx.pos);
  if (!math) return null;
  const word = cx.matchBefore(/\\[a-zA-Z]*/);
  // `\\` is a line break, not the start of a command.
  if (word && word.from >= math.from && cx.state.sliceDoc(word.from - 1, word.from) !== "\\") {
    if (word.text.length < 2 && !cx.explicit) return null;
    return { from: word.from, options: mathOptions(), validFor: /^\\[a-zA-Z]*$/ };
  }
  if (!cx.explicit) return null;
  return { from: cx.pos, options: mathOptions(), validFor: /^\\[a-zA-Z]*$/ };
}

/** `addToOptions` entry: a KaTeX preview left of each maths option. */
export const mathOptionPreview = {
  position: 10,
  render(completion: Completion): Node | null {
    const latex = optionPreviews.get(completion);
    if (latex == null) return null;
    const dom = el("span", "cm-math-option-preview");
    dom.innerHTML = previewHtml(latex);
    return dom;
  },
};

import { letterRunStart, matchBackward, tokenBefore } from "./scan";
import { FUNCTIONS, FUNCTION_ENDS, GREEK, OPERATORS, POWERS, SNIPPET_WORDS, TALL, WORD_OPERATORS } from "./tables";

/** A rewrite of the text before the caret: plain changes (`glue` when it
 *  ends in a control word) or a snippet replacing `from` to the caret. */
export type Rewrite =
  | { changes: { from: number; to?: number; insert: string }[]; glue?: boolean }
  | { from: number; template: string };

/** `s` is the maths from its start to the caret, the typed character last. */
type Rule = (s: string) => Rewrite | null;

const replaceEnd = (s: string, length: number, insert: string, glue = false): Rewrite => ({
  changes: [{ from: s.length - length, to: s.length, insert }],
  glue,
});

const escapeTemplate = (latex: string) => latex.replace(/[{}]/g, "\\$&");

const greek: Rule = (s) => {
  const m = /(^|[^\\])@(v?[A-Za-z])$/.exec(s);
  const name = m && GREEK[m[2]];
  return name ? replaceEnd(s, m[2].length + 1, `\\${name}`, true) : null;
};

const snippetWord: Rule = (s) => {
  const start = letterRunStart(s, s.length);
  const template = SNIPPET_WORDS[s.slice(start)];
  if (!template || s[start - 1] === "\\") return null;
  return { from: start, template };
};

const power: Rule = (s) => {
  const run = letterRunStart(s, s.length);
  if (s[run - 1] === "\\") return null;
  for (const [trigger, insert] of Object.entries(POWERS)) {
    if (!s.endsWith(trigger)) continue;
    const at = s.length - trigger.length;
    // The trigger alone, or after one letter that is its base (`xsr`).
    if (at - run > 1) return null;
    let baseEnd = at;
    while (s[baseEnd - 1] === " " || s[baseEnd - 1] === "\t") baseEnd--;
    const base = tokenBefore(s, baseEnd);
    if (!base || base.superscript) return null;
    return { from: baseEnd, template: insert };
  }
  return null;
};

const fraction: Rule = (s) => {
  if (!s.endsWith("/")) return null;
  const end = s.length - 1;
  if (s[end - 1] === "/") return { from: end - 1, template: "\\frac{#{1}}{#{2}}#{0}" };
  const token = tokenBefore(s, end);
  if (!token) return null;
  return { from: token.start, template: `\\frac{${escapeTemplate(token.numerator)}}{#{1}}#{0}` };
};

const operator: Rule = (s) => {
  for (const [trigger, insert] of OPERATORS) {
    if (!s.endsWith(trigger)) continue;
    const before = s[s.length - trigger.length - 1];
    if (before === "\\" || before === "^" || before === "_") return null;
    // `n!=` is a factorial: `!=` needs a space before it.
    if (trigger === "!=" && before !== undefined && !/\s/.test(before)) return null;
    return replaceEnd(s, trigger.length, insert, true);
  }
  const run = letterRunStart(s, s.length);
  const word = WORD_OPERATORS[s.slice(run)];
  if (!word || s[run - 1] === "\\") return null;
  return replaceEnd(s, s.length - run, word, true);
};

const functionWord: Rule = (s) => {
  const typed = s[s.length - 1];
  if (!FUNCTION_ENDS.has(typed)) return null;
  const end = s.length - 1;
  const start = letterRunStart(s, end);
  const word = s.slice(start, end);
  if (!FUNCTIONS.has(word)) return null;
  // Not a command, nor a name in a subscript (`x_{max}`).
  const before = s[start - 1];
  if (before === "\\" || before === "^" || before === "_") return null;
  if (before === "{" && (s[start - 2] === "^" || s[start - 2] === "_")) return null;
  if (word === "lim" && typed === " ") return { from: start, template: "\\lim_{#{1} \\to #{2}}#{0}" };
  return { changes: [{ from: start, insert: "\\" }] };
};

const enlarge: Rule = (s) => {
  const close = s.length - 1;
  const open = s[close] === ")" ? "(" : s[close] === "]" ? "[" : null;
  if (!open) return null;
  const sized = /\\(?:left|right|middle|[bB]igg?[lr]?)$/;
  if (sized.test(s.slice(0, close))) return null;
  const at = matchBackward(s, close, open);
  if (at < 0) return null;
  const lead = s.slice(Math.max(0, at - 16), at);
  // `[` after any command is an optional argument (`\sqrt[3]`).
  if (sized.test(lead) || (open === "[" && /\\[A-Za-z]+$/.test(lead))) return null;
  if (!TALL.test(s.slice(at + 1, close))) return null;
  return {
    changes: [
      { from: at, insert: "\\left" },
      { from: close, insert: "\\right" },
    ],
  };
};

/** Tried in order; the first rewrite wins. */
export const RULES: Rule[] = [greek, snippetWord, power, fraction, operator, functionWord, enlarge];

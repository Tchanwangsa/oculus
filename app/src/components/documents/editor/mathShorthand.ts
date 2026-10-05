import { snippet } from "@codemirror/autocomplete";
import { isolateHistory } from "@codemirror/commands";
import {
  StateEffect,
  StateField,
  type EditorState,
  type Extension,
  type Transaction,
  type TransactionSpec,
} from "@codemirror/state";
import { EditorView } from "@codemirror/view";

import { mathAt } from "./mathContext";

/**
 * LaTeX shorthand in maths, Obsidian Latex Suite style: typing `@a`, `a/`,
 * `xsr`, `->`, `sin ` … rewrites the LaTeX before the caret. Fires only on a
 * typed character inside maths and outside `\text{}`-like arguments.
 *
 * The typed character goes in as its own transaction, then the rewrite as a
 * second one isolated in history, so ⌘Z restores exactly what was typed.
 * Rewrites with slots are `snippet()`s: Tab / Shift-Tab move between them and
 * the last Tab leaves the group. Their fields are numbered, `#{0}` being the
 * exit, because `snippet()` sorts a numbered field before unnumbered ones.
 */

// ── Rule tables ───────────────────────────────────────────────────────────

/** `@<key>` → Greek letter. */
export const GREEK: Record<string, string> = {
  a: "alpha", b: "beta", g: "gamma", G: "Gamma", d: "delta", D: "Delta",
  e: "epsilon", ve: "varepsilon", z: "zeta", h: "eta", t: "theta", T: "Theta",
  vt: "vartheta", i: "iota", k: "kappa", l: "lambda", L: "Lambda", m: "mu",
  n: "nu", x: "xi", X: "Xi", p: "pi", P: "Pi", r: "rho", s: "sigma",
  S: "Sigma", u: "upsilon", f: "phi", vf: "varphi", F: "Phi", c: "chi",
  y: "psi", Y: "Psi", o: "omega", O: "Omega",
};

/** Letters typed after a base (`x`, `2`, `)`, `\alpha`) → its superscript. */
export const POWERS: Record<string, string> = {
  sr: "^2",
  cb: "^3",
  rd: "^{#{1}}#{0}",
  invs: "^{-1}",
};

/** Symbol runs → operators, longest first. `\le>` is `<=` already expanded. */
export const OPERATORS: [string, string][] = [
  ["<->", "\\leftrightarrow"],
  ["<=>", "\\iff"],
  ["\\le>", "\\iff"],
  ["->", "\\to"],
  ["=>", "\\implies"],
  ["<=", "\\le"],
  [">=", "\\ge"],
  ["!=", "\\ne"],
  ["~~", "\\approx"],
  ["...", "\\dots"],
  ["**", "\\cdot"],
];

/** Letter runs → operators, only when the whole run is the trigger. */
const WORD_OPERATORS: Record<string, string> = { ooo: "\\infty", xx: "\\times" };

/** Bare words that get a backslash once the next character ends them. */
const FUNCTIONS = new Set([
  "sin", "cos", "tan", "sec", "csc", "cot", "arcsin", "arccos", "arctan",
  "sinh", "cosh", "tanh", "log", "ln", "exp", "lim", "limsup", "liminf",
  "max", "min", "det", "sup", "inf", "gcd", "sum", "prod", "int",
]);
/** Characters that end a function word. */
const FUNCTION_ENDS = new Set([" ", "(", "^", "_", "\\"]);

/** Bare words that expand the moment they are complete, as snippets. */
const SNIPPET_WORDS: Record<string, string> = { sqrt: "\\sqrt{#{1}}#{0}" };

/** `)` / `]` around one of these becomes `\left( … \right)`. */
const TALL = /\\(?:[dt]?frac|binom|sum|prod|i?int|oint|begin)(?![A-Za-z])/;

/** Arguments that hold text or names, where no rule fires. */
const TEXT_ARGUMENT =
  /\\(?:text(?:bf|it|tt|rm|sf|color)?|math(?:rm|it|sf|tt|cal|bb|frak|scr)|operatorname\*?|mbox|begin|end|label|(?:eq)?ref|tag|color|href|url)\s*$/;

// ── Scanning ──────────────────────────────────────────────────────────────

const isLetter = (c: string | undefined) => !!c && /[A-Za-z]/.test(c);
const isDigit = (c: string | undefined) => !!c && c >= "0" && c <= "9";

/** Whether the end of `s` sits inside a `\text{…}`-like argument. */
function inTextArgument(s: string): boolean {
  const stack: boolean[] = [];
  for (let i = 0; i < s.length; i++) {
    const c = s[i];
    if (c === "\\") i++;
    else if (c === "{") {
      stack.push(stack[stack.length - 1] || TEXT_ARGUMENT.test(s.slice(Math.max(0, i - 24), i)));
    } else if (c === "}") stack.pop();
  }
  return stack[stack.length - 1] ?? false;
}

/** Index of the opener matching the closer at `s[close]`, or -1. Escaped
 *  braces don't count. */
function matchBackward(s: string, close: number, open: string): number {
  const shut = s[close];
  let depth = 0;
  for (let i = close; i >= 0; i--) {
    if (s[i - 1] === "\\" && (s[i] === "{" || s[i] === "}")) continue;
    if (s[i] === shut) depth++;
    else if (s[i] === open && --depth === 0) return i;
  }
  return -1;
}

/** Start of the letter run ending at `end` (exclusive). */
function letterRunStart(s: string, end: number): number {
  let i = end;
  while (isLetter(s[i - 1])) i--;
  return i;
}

interface Token {
  start: number;
  /** The LaTeX a fraction takes as its numerator: a lone `(…)` or `{…}`
   *  group loses its delimiters. */
  numerator: string;
  superscript: boolean;
}

/**
 * The operand ending at `end`: a number, a letter run (`2x`, `dy`), a
 * `\command` with its `{…}` / `[…]` arguments, a `(…)` group (with any
 * `\left`/`\right` or function name before it), each followed by any
 * `^`/`_` scripts. Null when none ends there or it is itself a script.
 */
function tokenBefore(s: string, end: number): Token | null {
  let i = end;
  let superscript = false;
  let scripted = false;
  for (;;) {
    if (s[i - 1] === "}") {
      const o = matchBackward(s, i - 1, "{");
      if (o > 0 && (s[o - 1] === "^" || s[o - 1] === "_")) {
        superscript ||= s[o - 1] === "^";
        i = o - 1;
        scripted = true;
        continue;
      }
    }
    if (/[A-Za-z0-9]/.test(s[i - 1] ?? "") && (s[i - 2] === "^" || s[i - 2] === "_")) {
      superscript ||= s[i - 2] === "^";
      i -= 2;
      scripted = true;
      continue;
    }
    const cmd = /[\^_]\\[A-Za-z]+$/.exec(s.slice(Math.max(0, i - 32), i));
    if (cmd) {
      superscript ||= cmd[0][0] === "^";
      i -= cmd[0].length;
      scripted = true;
      continue;
    }
    break;
  }
  const atomEnd = i;
  let start: number;
  let strip = false;
  const last = s[i - 1];
  if (last === ")") {
    start = matchBackward(s, i - 1, "(");
    if (start < 0) return null;
    if (start >= 5 && s.slice(start - 5, start) === "\\left") start -= 5;
    else {
      const name = letterRunStart(s, start);
      if (name < start) start = s[name - 1] === "\\" ? name - 1 : name;
      else strip = true;
    }
  } else if (last === "}") {
    start = matchBackward(s, i - 1, "{");
    if (start < 0) return null;
    // Further arguments: `\frac{a}{b}`, `\sqrt[3]{x}`.
    for (;;) {
      if (s[start - 1] === "}") start = matchBackward(s, start - 1, "{");
      else if (s[start - 1] === "]") start = matchBackward(s, start - 1, "[");
      else break;
      if (start < 0) return null;
    }
    const name = letterRunStart(s, start);
    if (name < start && s[name - 1] === "\\") start = name - 1;
    else if (name === start) strip = true;
    else return null;
  } else if (isDigit(last)) {
    start = i;
    while (isDigit(s[start - 1])) start--;
    if (s[start - 1] === "." && isDigit(s[start - 2])) {
      start--;
      while (isDigit(s[start - 1])) start--;
    }
  } else if (isLetter(last)) {
    start = letterRunStart(s, i);
    if (s[start - 1] === "\\") start--;
    else while (isDigit(s[start - 1])) start--;
  } else return null;
  if (start < 0) return null;
  const before = s[start - 1];
  if (before === "^" || before === "_" || before === "\\") return null;
  const text = s.slice(start, end);
  if (text.includes("\n")) return null;
  const numerator = strip && !scripted ? s.slice(start + 1, atomEnd - 1) : text;
  return { start, numerator, superscript };
}

// ── Rules ─────────────────────────────────────────────────────────────────

/** A rewrite of the text before the caret: plain changes (`glue` when it
 *  ends in a control word) or a snippet replacing `from` to the caret. */
type Rewrite =
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
const RULES: Rule[] = [greek, snippetWord, power, fraction, operator, functionWord, enlarge];

// ── Transactions ──────────────────────────────────────────────────────────

/** End of the last control word a rewrite inserted, while the caret has not
 *  moved off it: a letter typed there gets a space so it can't extend it. */
const setGlueEnd = StateEffect.define<number>();
const glueEnd = StateField.define<number | null>({
  create: () => null,
  update(value, tr) {
    for (const e of tr.effects) if (e.is(setGlueEnd)) return e.value;
    return tr.docChanged || tr.selection ? null : value;
  },
});

const expansion = { userEvent: "input.complete", annotations: isolateHistory.of("full") };

/** Longest maths prefix a rule scans; no rule reaches further back. */
const MAX_SCAN = 20000;

/**
 * The rewrite for a character just typed, `pos` being the caret after it and
 * `math` the LaTeX bounds it was typed in. Null when no rule fires.
 */
export function expand(
  state: EditorState,
  pos: number,
  math: { from: number; to: number },
): TransactionSpec | null {
  if (pos <= math.from || pos > math.to) return null;
  const base = Math.max(math.from, pos - MAX_SCAN);
  const s = state.sliceDoc(base, pos);
  if (inTextArgument(s)) return null;
  for (const rule of RULES) {
    const rewrite = rule(s);
    if (!rewrite) continue;
    if ("template" in rewrite) {
      // Take the snippet's transaction apart to dispatch it as an expansion.
      const built: Transaction[] = [];
      snippet(rewrite.template)({ state, dispatch: (tr) => built.push(tr) }, null, base + rewrite.from, pos);
      const t = built[0];
      if (!t) return null;
      return { changes: t.changes, selection: t.selection, effects: t.effects, scrollIntoView: true, ...expansion };
    }
    const changes = state.changes(
      rewrite.changes.map(({ from, to, insert }) => ({ from: base + from, to: base + (to ?? from), insert })),
    );
    const head = changes.mapPos(pos, 1);
    return {
      changes,
      selection: { anchor: head },
      effects: rewrite.glue ? setGlueEnd.of(head) : [],
      scrollIntoView: true,
      ...expansion,
    };
  }
  return null;
}

/** A letter typed straight after an inserted control word, with the space
 *  that keeps `\alpha` + `x` from reading as `\alphax`. */
export function spacedInput(state: EditorState, from: number, to: number, text: string): TransactionSpec | null {
  if (from !== to || !/^[A-Za-z]$/.test(text) || state.field(glueEnd, false) !== from) return null;
  return {
    changes: { from, insert: ` ${text}` },
    selection: { anchor: from + text.length + 1 },
    scrollIntoView: true,
    userEvent: "input.type",
  };
}

export function mathShorthand(): Extension {
  return [
    glueEnd,
    EditorView.inputHandler.of((view, from, to, text, insert) => {
      const { state } = view;
      if (view.composing || view.compositionStarted || state.readOnly) return false;
      if (text.length !== 1 || from !== to || state.selection.ranges.length > 1) return false;
      const spaced = spacedInput(state, from, to, text);
      if (spaced) {
        view.dispatch(spaced);
        return true;
      }
      // Judged before the insert: a typed space can unmake `$…$` maths.
      const math = mathAt(state, from);
      if (!math) return false;
      const typed = insert();
      view.dispatch(typed);
      const pos = view.state.selection.main.head;
      const bounds = { from: typed.changes.mapPos(math.from, -1), to: typed.changes.mapPos(math.to, 1) };
      if (view.state === typed.state && pos === from + 1) {
        const spec = expand(view.state, pos, bounds);
        if (spec) view.dispatch(spec);
      }
      return true;
    }),
  ];
}

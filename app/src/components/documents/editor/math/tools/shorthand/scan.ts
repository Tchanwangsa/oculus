import { TEXT_ARGUMENT } from "./tables";

const isLetter = (c: string | undefined) => !!c && /[A-Za-z]/.test(c);
const isDigit = (c: string | undefined) => !!c && c >= "0" && c <= "9";

/** Whether the end of `s` sits inside a `\text{…}`-like argument. */
export function inTextArgument(s: string): boolean {
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
export function matchBackward(s: string, close: number, open: string): number {
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
export function letterRunStart(s: string, end: number): number {
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
export function tokenBefore(s: string, end: number): Token | null {
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

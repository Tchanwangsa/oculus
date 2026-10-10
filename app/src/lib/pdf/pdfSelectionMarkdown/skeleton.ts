import { normalizeText } from "@/lib/citations/text";

/** A half-open source span. */
export interface Span {
  start: number;
  end: number;
}

/** A source string's comparison form: per char of `text` (UTF-16 units), the
 *  source span it came from. */
export interface Skeleton {
  text: string;
  from: number[];
  to: number[];
}

/** Copied whole or not at all: maths, an image (`url` is its link target, for
 *  rewriting), a link, a code span, an HTML table. */
export interface Atom extends Span {
  url?: Span;
}

/** A matched emphasis delimiter pair (`*`, `**`, `_`, `__`, `~`, `~~`). */
export interface Pair {
  open: Span;
  close: Span;
}

export interface MdSkeleton extends Skeleton {
  atoms: Atom[];
  pairs: Pair[];
}

const folded = new Map<string, string>();

/** One code point in comparison form; most are ASCII, the rest are cached. */
function fold(ch: string): string {
  const c = ch.charCodeAt(0);
  if (ch.length === 1 && c < 0x80) {
    if ((c >= 48 && c <= 57) || (c >= 97 && c <= 122)) return ch;
    if (c >= 65 && c <= 90) return String.fromCharCode(c + 32);
    return "";
  }
  let f = folded.get(ch);
  if (f === undefined) {
    f = normalizeText(ch);
    folded.set(ch, f);
  }
  return f;
}

function codePointLength(s: string, i: number): number {
  const c = s.charCodeAt(i);
  return c >= 0xd800 && c <= 0xdbff && i + 1 < s.length ? 2 : 1;
}

/** Adds `ch`'s comparison form to `sk`, every char pointing at [a, b). */
function emit(sk: Skeleton, ch: string, a: number, b: number): void {
  const f = fold(ch);
  for (let k = 0; k < f.length; k++) {
    sk.text += f[k];
    sk.from.push(a);
    sk.to.push(b);
  }
}

/** Text-layer (or any plain) text in comparison form. */
export function textSkeleton(raw: string): Skeleton {
  const sk: Skeleton = { text: "", from: [], to: [] };
  for (let i = 0; i < raw.length; ) {
    const n = codePointLength(raw, i);
    emit(sk, raw.slice(i, i + n), i, i + n);
    i += n;
  }
  return sk;
}

/** TeX commands that print a letter. A PDF's maths reaches the text layer as
 *  Unicode (often math-italic, which `normalizeText` folds to plain). */
const TEX_LETTERS: Record<string, string> = {
  alpha: "α", beta: "β", gamma: "γ", delta: "δ", epsilon: "ε", varepsilon: "ε",
  zeta: "ζ", eta: "η", theta: "θ", vartheta: "θ", iota: "ι", kappa: "κ",
  lambda: "λ", mu: "μ", nu: "ν", xi: "ξ", pi: "π", varpi: "π", rho: "ρ",
  varrho: "ρ", sigma: "σ", varsigma: "ς", tau: "τ", upsilon: "υ", phi: "φ",
  varphi: "φ", chi: "χ", psi: "ψ", omega: "ω", Gamma: "Γ", Delta: "Δ",
  Theta: "Θ", Lambda: "Λ", Xi: "Ξ", Pi: "Π", Sigma: "Σ", Upsilon: "Υ",
  Phi: "Φ", Psi: "Ψ", Omega: "Ω", ell: "ℓ", imath: "i", jmath: "j", hbar: "ħ",
  aleph: "ℵ",
};

/** Commands whose `{…}` argument is never printed. */
const TEX_HIDDEN_ARG = new Set(["begin", "end", "label", "hspace", "vspace", "color", "textcolor"]);

/** Environments whose column spec (`\begin{array}{ll}`) is never printed. */
const TEX_COLUMN_SPEC = /^\{\s*(?:array|tabular)\*?\s*\}$/;

const ASCII_PUNCT = /[!-/:-@[-`{-~]/;
const WS = /\s/;
const PUNCT = /[\p{P}\p{S}]/u;
const BLANK_LINE = /\n[ \t]*\n/y;
const TAG = /<\/?[A-Za-z][A-Za-z0-9-]*(?:\s[^<>]*)?\/?>|<!--[\s\S]*?-->/y;
const ENTITY = /&(?:#\d+|#x[0-9a-f]+|[a-z][a-z0-9]*);/iy;
const TABLE_OPEN = /<table[\s>]/iy;

function at(re: RegExp, s: string, i: number): RegExpExecArray | null {
  re.lastIndex = i;
  return re.exec(s);
}

/** A delimiter run as CommonMark sees it: may it open, may it close. */
interface Delim {
  ch: string;
  start: number;
  end: number;
  open: boolean;
  close: boolean;
}

/** Pairs a paragraph's delimiter runs, innermost first (CommonMark's
 *  algorithm, minus the rule of three). */
function pairRuns(runs: Delim[], pairs: Pair[]): void {
  const stack: Delim[] = [];
  for (const r of runs) {
    const run = { ...r };
    if (run.close) {
      for (let k = stack.length - 1; k >= 0 && run.end > run.start; k--) {
        const op = stack[k];
        if (op.ch !== run.ch) continue;
        const opLen = op.end - op.start;
        const runLen = run.end - run.start;
        if (op.ch === "~" && opLen !== runLen) continue;
        const n = op.ch === "~" ? runLen : Math.min(2, opLen, runLen);
        pairs.push({
          open: { start: op.end - n, end: op.end },
          close: { start: run.start, end: run.start + n },
        });
        op.end -= n;
        run.start += n;
        // Openers between the two can no longer match anything.
        stack.length = op.end > op.start ? k + 1 : k;
        k = stack.length;
      }
    }
    if (run.open && run.end > run.start) stack.push(run);
  }
}

/** The index of the `]` closing the `[` at `i`, or -1. */
function closeBracket(md: string, i: number, limit: number): number {
  let depth = 0;
  for (let j = i; j < limit; j++) {
    const c = md[j];
    if (c === "\\") j++;
    else if (c === "[") depth++;
    else if (c === "]" && --depth === 0) return j;
    else if (c === "\n" && md[j + 1] === "\n") return -1;
  }
  return -1;
}

/** The index of the `)` closing the `(` at `i`, or -1. */
function closeParen(md: string, i: number, limit: number): number {
  let depth = 0;
  for (let j = i; j < limit; j++) {
    const c = md[j];
    if (c === "\\") j++;
    else if (c === "(") depth++;
    else if (c === ")" && --depth === 0) return j;
    else if (c === "\n") return -1;
  }
  return -1;
}

/** `[text](url)` at `i`: the `]` and `)` indices, or null. */
function linkAt(md: string, i: number, limit: number): { close: number; paren: number } | null {
  const close = closeBracket(md, i, limit);
  if (close < 0 || md[close + 1] !== "(") return null;
  const paren = closeParen(md, close + 1, limit);
  return paren < 0 ? null : { close, paren };
}

/** The end of an unescaped run of exactly `len` `ch`s at or after `i`, or -1;
 *  inline spans stop at a blank line. */
function closingRun(md: string, ch: string, len: number, i: number, limit: number, inline: boolean): number {
  for (let j = i; j < limit; j++) {
    const c = md[j];
    if (c === "\\" && ch === "$") {
      j++;
      continue;
    }
    if (inline && c === "\n" && at(BLANK_LINE, md, j)) return -1;
    if (c !== ch) continue;
    let k = j;
    while (k < limit && md[k] === ch) k++;
    if (k - j === len) return k;
    j = k - 1;
  }
  return -1;
}

/**
 * A page's markdown in comparison form, with its atoms and emphasis pairs.
 * Syntax adds nothing; link targets and HTML tags add nothing; an image's alt
 * text does (the caption is printed near the figure, so it anchors it); maths
 * adds its TeX minus command names, except those that print a letter.
 */
export function markdownSkeleton(md: string): MdSkeleton {
  const sk: MdSkeleton = { text: "", from: [], to: [], atoms: [], pairs: [] };
  let runs: Delim[] = [];
  const flush = () => {
    if (runs.length) pairRuns(runs, sk.pairs);
    runs = [];
  };

  const plain = (a: number, b: number) => {
    for (let i = a; i < b; ) {
      const n = codePointLength(md, i);
      emit(sk, md.slice(i, i + n), i, i + n);
      i += n;
    }
  };

  /** `{…}` at or after `i` (past spaces): its end, or `i` if there is none. */
  const group = (i: number, b: number): number => {
    let j = i;
    while (j < b && md[j] === " ") j++;
    if (md[j] !== "{") return i;
    let depth = 0;
    for (; j < b; j++) {
      if (md[j] === "\\") j++;
      else if (md[j] === "{") depth++;
      else if (md[j] === "}" && --depth === 0) return j + 1;
    }
    return b;
  };

  const tex = (a: number, b: number) => {
    for (let i = a; i < b; ) {
      if (md[i] !== "\\") {
        const n = codePointLength(md, i);
        emit(sk, md.slice(i, i + n), i, i + n);
        i += n;
        continue;
      }
      let end = i + 1;
      while (end < b && /[A-Za-z]/.test(md[end])) end++;
      if (end === i + 1) {
        i += 2;
        continue;
      }
      const name = md.slice(i + 1, end);
      const letter = TEX_LETTERS[name];
      if (letter) emit(sk, letter, i, end);
      i = end;
      if (TEX_HIDDEN_ARG.has(name)) {
        const after = group(i, b);
        if (name === "begin" && TEX_COLUMN_SPEC.test(md.slice(i, after).trim())) i = group(after, b);
        else i = after;
      }
    }
  };

  const scan = (a: number, b: number) => {
    let i = a;
    while (i < b) {
      const c = md[i];
      if (c === "\\" && i + 1 < b && ASCII_PUNCT.test(md[i + 1])) {
        // Escaped punctuation is literal, and folds to nothing.
        i += 2;
        continue;
      }
      if (c === "\n" && at(BLANK_LINE, md, i)) {
        flush();
        i++;
        continue;
      }
      if (c === "$") {
        let n = i;
        while (n < b && md[n] === "$") n++;
        const len = Math.min(n - i, 2);
        const end = closingRun(md, "$", len, i + len, b, len === 1);
        if (end < 0 || n - i > 2) {
          i = n;
          continue;
        }
        sk.atoms.push({ start: i, end });
        tex(i + len, end - len);
        i = end;
        continue;
      }
      if (c === "`") {
        let n = i;
        while (n < b && md[n] === "`") n++;
        const end = closingRun(md, "`", n - i, n, b, n - i < 3);
        if (end < 0) {
          i = n;
          continue;
        }
        sk.atoms.push({ start: i, end });
        plain(n, end - (n - i));
        i = end;
        continue;
      }
      if (c === "!" && md[i + 1] === "[") {
        const link = linkAt(md, i + 1, b);
        if (link) {
          let u0 = link.close + 2;
          let u1 = link.paren;
          while (u0 < u1 && md[u0] === " ") u0++;
          while (u1 > u0 && md[u1 - 1] === " ") u1--;
          sk.atoms.push({ start: i, end: link.paren + 1, url: { start: u0, end: u1 } });
          plain(i + 2, link.close);
          i = link.paren + 1;
          continue;
        }
      }
      if (c === "[") {
        const link = linkAt(md, i, b);
        if (link) {
          sk.atoms.push({ start: i, end: link.paren + 1 });
          plain(i + 1, link.close);
          i = link.paren + 1;
          continue;
        }
        i++;
        continue;
      }
      if (c === "<") {
        if (at(TABLE_OPEN, md, i)) {
          const close = md.toLowerCase().indexOf("</table>", i);
          const body = md.indexOf(">", i) + 1;
          if (close >= 0 && close < b) {
            const end = close + "</table>".length;
            flush();
            sk.atoms.push({ start: i, end });
            scan(body, close);
            flush();
            i = end;
            continue;
          }
        }
        const tag = at(TAG, md, i);
        i += tag ? tag[0].length : 1;
        continue;
      }
      if (c === "&") {
        const entity = at(ENTITY, md, i);
        i += entity ? entity[0].length : 1;
        continue;
      }
      if (c === "*" || c === "_" || c === "~") {
        let n = i;
        while (n < b && md[n] === c) n++;
        const prev = i > 0 ? md[i - 1] : "\n";
        const next = n < md.length ? md[n] : "\n";
        const left = !WS.test(next) && (!PUNCT.test(next) || WS.test(prev) || PUNCT.test(prev));
        const right = !WS.test(prev) && (!PUNCT.test(prev) || WS.test(next) || PUNCT.test(next));
        const open = c === "_" ? left && (!right || PUNCT.test(prev)) : left;
        const close = c === "_" ? right && (!left || PUNCT.test(next)) : right;
        if ((open || close) && (c !== "~" || n - i <= 2)) runs.push({ ch: c, start: i, end: n, open, close });
        i = n;
        continue;
      }
      const n = codePointLength(md, i);
      emit(sk, md.slice(i, i + n), i, i + n);
      i += n;
    }
  };

  scan(0, md.length);
  flush();
  return sk;
}

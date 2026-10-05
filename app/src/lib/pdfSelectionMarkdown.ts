import type { ClipboardEvent, DragEvent } from "react";
import { normalizeText } from "@/lib/citations";
import { normalizeMath } from "@/lib/mathMarkdown";
import { elementMarkdown } from "@/lib/selectionMarkdown";

/**
 * A selection in the PDF viewer, copied as the parsed markdown instead of
 * pdf.js's text layer. The pages' markdown is one document (`parsedDoc`); a
 * page's text-layer text and its markdown are reduced to one comparison form
 * (`normalizeText`: letters and digits) and aligned patience-diff style, so
 * the selection's two ends land in the document and everything between is one
 * slice — whole pages, unrendered ones and text MinerU filed under the
 * neighbouring page included. The slice is snapped outward so maths, figures,
 * tables, links and code come whole and emphasis stays balanced. An end on a
 * page with no markdown, or one too poorly aligned to trust, copies that
 * page's text. The core is pure (tested under bun, which has no DOM); the DOM
 * edge at the bottom reads pdf.js's `.page` / `.textLayer` markup.
 */

// ── Skeletons ────────────────────────────────────────────────────────────────

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

// ── Alignment ────────────────────────────────────────────────────────────────

/** `t[t, t+len)` equals `m[m, m+len)`. */
export interface Match {
  t: number;
  m: number;
  len: number;
}

export interface Alignment {
  /** Increasing in both strings, never overlapping. */
  runs: Match[];
  tLen: number;
  mLen: number;
  /** The share of the shorter string inside runs of at least `GRAM` chars. */
  coverage: number;
}

/** Anchor length. On the fixtures 6–12 all score within a few windows of
 *  each other; shorter grams also tried inside gaps made garbled maths match
 *  by chance, so a gap is re-aligned with grams unique inside it instead. */
const GRAM = 8;

/** Below this share of the shorter side in runs of `GRAM`+ chars, the page's
 *  mapping is guesswork (see `align`). */
export const MIN_COVERAGE = 0.3;

/** A text layer this short is matched whole or not at all. */
const SHORT = 2 * GRAM;

/** Indices of the longest chain of anchors increasing in `m` (`t` already
 *  increases): patience sorting. */
function longestIncreasing(ms: number[]): number[] {
  const tails: number[] = [];
  const prev = new Array<number>(ms.length);
  for (let k = 0; k < ms.length; k++) {
    let lo = 0;
    let hi = tails.length;
    while (lo < hi) {
      const mid = (lo + hi) >> 1;
      if (ms[tails[mid]] < ms[k]) lo = mid + 1;
      else hi = mid;
    }
    prev[k] = lo > 0 ? tails[lo - 1] : -1;
    tails[lo] = k;
  }
  const out: number[] = [];
  for (let k = tails.length ? tails[tails.length - 1] : -1; k >= 0; k = prev[k]) out.push(k);
  return out.reverse();
}

/** Per `k`-gram of `s[a, b)`, its only start, or -1 when it repeats. */
function uniqueGrams(s: string, a: number, b: number, k: number): Map<string, number> {
  const out = new Map<string, number>();
  for (let i = a; i + k <= b; i++) {
    const g = s.slice(i, i + k);
    out.set(g, out.has(g) ? -1 : i);
  }
  return out;
}

/** Aligns `t[t0, t1)` to `m[m0, m1)`, appending runs to `out` in order:
 *  patience diff, recursing into the gaps between its runs. */
function alignGap(t: string, m: string, t0: number, t1: number, m0: number, m1: number, out: Match[]): void {
  const k = GRAM;
  if (t1 - t0 < k || m1 - m0 < k) return;
  const tg = uniqueGrams(t, t0, t1, k);
  const mg = uniqueGrams(m, m0, m1, k);
  const ts: number[] = [];
  const ms: number[] = [];
  for (let i = t0; i + k <= t1; i++) {
    const g = t.slice(i, i + k);
    if (tg.get(g) !== i) continue;
    const j = mg.get(g);
    if (j === undefined || j < 0) continue;
    ts.push(i);
    ms.push(j);
  }
  if (!ts.length) return;

  // Chain → runs, merging anchors on one diagonal, trimming overlaps.
  const runs: Match[] = [];
  for (const c of longestIncreasing(ms)) {
    let rt = ts[c];
    let rm = ms[c];
    let len = k;
    const last = runs[runs.length - 1];
    if (last && rt - last.t === rm - last.m && rt <= last.t + last.len) {
      last.len = Math.max(last.len, rt + len - last.t);
      continue;
    }
    if (last) {
      const cut = Math.max(last.t + last.len - rt, last.m + last.len - rm, 0);
      rt += cut;
      rm += cut;
      len -= cut;
      if (len <= 0) continue;
    }
    runs.push({ t: rt, m: rm, len });
  }

  // Grow each run outward while the chars agree, up to its neighbours.
  for (let r = 0; r < runs.length; r++) {
    const run = runs[r];
    const lo = runs[r - 1];
    const loT = lo ? lo.t + lo.len : t0;
    const loM = lo ? lo.m + lo.len : m0;
    while (run.t > loT && run.m > loM && t[run.t - 1] === m[run.m - 1]) {
      run.t--;
      run.m--;
      run.len++;
    }
    const hi = runs[r + 1];
    const hiT = hi ? hi.t : t1;
    const hiM = hi ? hi.m : m1;
    while (run.t + run.len < hiT && run.m + run.len < hiM && t[run.t + run.len] === m[run.m + run.len]) {
      run.len++;
    }
  }

  let pt = t0;
  let pm = m0;
  for (const run of runs) {
    alignGap(t, m, pt, run.t, pm, run.m, out);
    out.push(run);
    pt = run.t + run.len;
    pm = run.m + run.len;
  }
  alignGap(t, m, pt, t1, pm, m1, out);
}

/**
 * A monotonic alignment of text-layer skeleton `t` to markdown skeleton `m`:
 * grams unique to both anchor it, the longest increasing chain of anchors
 * wins, each run is grown while the chars agree, and the gaps are aligned the
 * same way. Null when less than `minCoverage` of the shorter string is in
 * runs of `GRAM`+ chars — the page then copies as plain text.
 */
export function align(t: string, m: string, minCoverage = MIN_COVERAGE): Alignment | null {
  if (!t.length || !m.length) return null;
  if (t.length < SHORT) {
    const i = m.indexOf(t);
    if (i < 0 || m.indexOf(t, i + 1) >= 0) return null;
    return { runs: [{ t: 0, m: i, len: t.length }], tLen: t.length, mLen: m.length, coverage: 1 };
  }
  const runs: Match[] = [];
  alignGap(t, m, 0, t.length, 0, m.length, runs);
  // The shorter side: a text layer can carry text the markdown never had
  // (embedded TeX source, a diagram's labels) and the markdown OCR'd text.
  const strong = runs.reduce((n, r) => (r.len >= GRAM ? n + r.len : n), 0);
  const coverage = strong / Math.min(t.length, m.length);
  if (coverage < minCoverage) return null;
  return { runs, tLen: t.length, mLen: m.length, coverage };
}

/** A boundary in `t` (0…tLen) carried to `m`: exact inside a run,
 *  proportional across a gap. */
export function mapBoundary(al: Alignment, p: number): number {
  const { runs } = al;
  let lo = 0;
  let hi = runs.length;
  // The first run starting after `p`.
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if (runs[mid].t <= p) lo = mid + 1;
    else hi = mid;
  }
  const before = runs[lo - 1];
  if (before && p <= before.t + before.len) return before.m + (p - before.t);
  const pt = before ? before.t + before.len : 0;
  const pm = before ? before.m + before.len : 0;
  const after = runs[lo];
  const nt = after ? after.t : al.tLen;
  const nm = after ? after.m : al.mLen;
  if (nt === pt) return pm;
  return pm + Math.round(((p - pt) * (nm - pm)) / (nt - pt));
}

// ── Slicing ──────────────────────────────────────────────────────────────────

/** What may stand between a line's start and its first selected char without
 *  being content: a heading, list or quote marker, a printed bullet. */
const BLOCK_PREFIX = /^[ \t]*(?:>[ \t]*)*(?:#{1,6}[ \t]+|[-*+•][ \t]+|\d{1,9}[.)][ \t]+)?$/;

/**
 * `md[S, E)` snapped outward: atoms whole, a split delimiter whole, a line's
 * block prefix kept; an emphasis pair cut by an end is reopened or closed at
 * it. Image targets go through `resolveImage`.
 */
export function sliceMarkdown(
  md: string,
  sk: MdSkeleton,
  S: number,
  E: number,
  resolveImage: (src: string) => string = (src) => src,
): string {
  if (S >= E) return "";

  // Atoms and pairs both move S left and E right; repeat until neither does.
  for (let changed = true; changed; ) {
    changed = false;
    for (const atom of sk.atoms) {
      if (atom.start >= E || atom.end <= S) continue;
      if (atom.start < S) {
        S = atom.start;
        changed = true;
      }
      if (atom.end > E) {
        E = atom.end;
        changed = true;
      }
    }
    for (const p of sk.pairs) {
      if (S >= p.close.start || E <= p.open.end) continue;
      if (S > p.open.start && S <= p.open.end) {
        S = p.open.start;
        changed = true;
      }
      if (E < p.close.end && E >= p.close.start) {
        E = p.close.end;
        changed = true;
      }
    }
  }

  const lineStart = md.lastIndexOf("\n", S - 1) + 1;
  if (BLOCK_PREFIX.test(md.slice(lineStart, S))) S = lineStart;

  // Pairs the slice cuts through, reopened before it and closed after it.
  const reopen: Pair[] = [];
  const reclose: Pair[] = [];
  for (const p of sk.pairs) {
    if (S >= p.close.start || E <= p.open.end) continue;
    if (S > p.open.start) reopen.push(p);
    if (E < p.close.end) reclose.push(p);
  }
  reopen.sort((x, y) => x.open.start - y.open.start);
  reclose.sort((x, y) => x.close.start - y.close.start);

  let body = "";
  let cursor = S;
  for (const atom of sk.atoms) {
    if (!atom.url || atom.start < S || atom.end > E || atom.url.start < cursor) continue;
    body += md.slice(cursor, atom.url.start) + resolveImage(md.slice(atom.url.start, atom.url.end));
    cursor = atom.url.end;
  }
  body = (body + md.slice(cursor, E)).trim();
  if (!body) return "";
  const prefix = reopen.map((p) => md.slice(p.open.start, p.open.end)).join("");
  const suffix = reclose.map((p) => md.slice(p.close.start, p.close.end)).join("");
  return prefix + body + suffix;
}

/** How many skeleton chars start before source offset `p`. */
function charsBefore(sk: Skeleton, p: number): number {
  let lo = 0;
  let hi = sk.from.length;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if (sk.from[mid] < p) lo = mid + 1;
    else hi = mid;
  }
  return lo;
}

// ── The document ─────────────────────────────────────────────────────────────

/** A page's slot in the document: source and skeleton spans. */
export interface DocPage {
  start: number;
  end: number;
  skStart: number;
  skEnd: number;
  parsed: boolean;
}

/** The parsed pages as one markdown string (joined like the `.md` file), so a
 *  selection over several pages is one contiguous slice. */
export interface ParsedDoc {
  md: string;
  sk: MdSkeleton;
  pages: Map<number, DocPage>;
}

const docs = new WeakMap<ReadonlyMap<number, string>, ParsedDoc>();

/** `pages` (page_no → markdown) as a document, cached per map. Each page is
 *  scanned on its own, so a stray delimiter cannot pair across pages. */
export function parsedDoc(pages: ReadonlyMap<number, string>): ParsedDoc {
  const hit = docs.get(pages);
  if (hit) return hit;
  const doc: ParsedDoc = { md: "", sk: { text: "", from: [], to: [], atoms: [], pairs: [] }, pages: new Map() };
  const last = Math.max(0, ...pages.keys());
  const shift = (sp: Span, by: number): Span => ({ start: sp.start + by, end: sp.end + by });
  for (let n = 1; n <= last; n++) {
    if (n > 1) doc.md += "\n\n";
    const md = pages.get(n);
    const base = doc.md.length;
    const skStart = doc.sk.text.length;
    if (md !== undefined) {
      const sk = markdownSkeleton(md);
      doc.md += md;
      doc.sk.text += sk.text;
      for (const f of sk.from) doc.sk.from.push(f + base);
      for (const t of sk.to) doc.sk.to.push(t + base);
      for (const a of sk.atoms) doc.sk.atoms.push({ ...shift(a, base), url: a.url && shift(a.url, base) });
      for (const p of sk.pairs) doc.sk.pairs.push({ open: shift(p.open, base), close: shift(p.close, base) });
    }
    doc.pages.set(n, { start: base, end: doc.md.length, skStart, skEnd: doc.sk.text.length, parsed: md !== undefined });
  }
  docs.set(pages, doc);
  return doc;
}

/** Text carried onto the next PDF page: MinerU files a paragraph under the
 *  page it starts on. A page's unmatched lead (trail) is looked for at the end
 *  (start) of the previous (next) page's markdown, within this many skeleton
 *  chars of it (a paper's page number and margin line numbers come to ~150)… */
const SPILL_SLACK = 200;

/** …with at least this share of it matched. */
const SPILL_COVERAGE = 0.5;

/** An alignment of `t[offset, …)` to the document's skeleton from `base`. */
interface Region {
  al: Alignment;
  offset: number;
  base: number;
}

/** A page's text layer mapped onto the document. */
interface PageMap {
  main: Region;
  lead: Region | null;
  trail: Region | null;
}

function runsEnd(al: Alignment): number {
  const last = al.runs[al.runs.length - 1];
  return last ? last.t + last.len : 0;
}

/** Like `mapBoundary`, but held to the matched span: outside it is text the
 *  region does not have (a running header above a carried paragraph). */
function mapClamped(al: Alignment, p: number): number {
  const first = al.runs[0];
  const last = al.runs[al.runs.length - 1];
  if (p <= first.t) return first.m;
  if (p >= last.t + last.len) return last.m + last.len;
  return mapBoundary(al, p);
}

function pageMap(doc: ParsedDoc, n: number, t: Skeleton): PageMap | null {
  const page = doc.pages.get(n);
  if (!page?.parsed || !t.text.length) return null;
  const text = doc.sk.text;
  const window = (a: DocPage, b: DocPage) => text.slice(a.skStart, b.skEnd);
  const prev = doc.pages.get(n - 1);
  const next = doc.pages.get(n + 1);

  const own = align(t.text, window(page, page));
  if (!own) {
    // A page made mostly of carried text: align against its neighbours too.
    const lo = prev ?? page;
    const wide = align(t.text, window(lo, next ?? page));
    return wide && { main: { al: wide, offset: 0, base: lo.skStart }, lead: null, trail: null };
  }
  const main: Region = { al: own, offset: 0, base: page.skStart };
  let lead: Region | null = null;
  let trail: Region | null = null;
  const firstT = own.runs.length ? own.runs[0].t : t.text.length;
  if (prev?.parsed && firstT >= SHORT) {
    const m = window(prev, prev);
    const al = align(t.text.slice(0, firstT), m, SPILL_COVERAGE);
    if (al?.runs.length && m.length - (al.runs[al.runs.length - 1].m + al.runs[al.runs.length - 1].len) <= SPILL_SLACK)
      lead = { al, offset: 0, base: prev.skStart };
  }
  const lastT = runsEnd(own);
  if (next?.parsed && t.text.length - lastT >= SHORT) {
    const al = align(t.text.slice(lastT), window(next, next), SPILL_COVERAGE);
    if (al?.runs.length && al.runs[0].m <= SPILL_SLACK) trail = { al, offset: lastT, base: next.skStart };
  }
  return { main, lead, trail };
}

/** A text-layer boundary carried to a document skeleton boundary. */
function toDoc(pm: PageMap, p: number): number {
  const { main, lead, trail } = pm;
  if (lead && p < main.al.runs[0].t) return lead.base + mapClamped(lead.al, p - lead.offset);
  if (trail && p > runsEnd(main.al)) return trail.base + mapClamped(trail.al, p - trail.offset);
  return main.base + mapBoundary(main.al, p - main.offset);
}

/** Source offset of skeleton boundary `b` read as a start: its next char. */
function startAt(doc: ParsedDoc, b: number): number {
  return b >= doc.sk.text.length ? doc.md.length : doc.sk.from[b];
}

/** …read as an end: just past its previous char. */
function endAt(doc: ParsedDoc, b: number): number {
  return b <= 0 ? 0 : doc.sk.to[b - 1];
}

/** Where the page's own text begins in the document: before any of its
 *  markdown, or where its carried-over lead begins. */
function pageStart(doc: ParsedDoc, page: DocPage, pm: PageMap | null): number {
  return pm?.lead ? startAt(doc, pm.lead.base + pm.lead.al.runs[0].m) : page.start;
}

/** Where it ends: after all of its markdown (a trailing figure included), or
 *  where its carried-on trail ends. */
function pageEnd(doc: ParsedDoc, page: DocPage, pm: PageMap | null): number {
  return pm?.trail ? endAt(doc, pm.trail.base + runsEnd(pm.trail.al)) : page.end;
}

/** One page's part in a selection. */
export interface PageSelection {
  page: number;
  /** The page's text-layer text, or null when it is not rendered. */
  layer: string | null;
  /** Offsets into `layer`; null for the page's own start / end. */
  start: number | null;
  end: number | null;
}

/** A page copied as its text-layer text, among markdown pages: pdf.js's own
 *  clean-up (no NULs or controls, ligatures split), and `$` escaped so it
 *  cannot pair into maths. */
export function plainText(text: string): string {
  return text
    .replace(/[\x00-\x08\x0b\x0c\x0e-\x1f]/g, "")
    .replace(/[\ufb00-\ufb06]/g, (c) => c.normalize("NFKC"))
    .replace(/\$/g, "\\$")
    .trim();
}

/**
 * A selection over `sel` (its pages in order; only the first and last may be
 * partial) as markdown, or "" when no part of it could be read from `doc`.
 * The ends are carried to the document and everything between is one slice;
 * an end on a page that is unparsed or cannot be aligned copies that page's
 * selected text instead.
 */
export function markdownOfSelection(
  doc: ParsedDoc,
  sel: PageSelection[],
  resolveImage: (src: string) => string = (src) => src,
): string {
  const parts: string[] = [];
  let parsed = false;
  let cursor: number | null = null;
  const take = (end: number) => {
    if (cursor !== null && end > cursor) {
      const md = sliceMarkdown(doc.md, doc.sk, cursor, end, resolveImage);
      if (md) {
        parts.push(md);
        parsed = true;
      }
    }
    cursor = null;
  };
  const plain = (p: PageSelection) => {
    if (p.layer) parts.push(plainText(p.layer.slice(p.start ?? 0, p.end ?? p.layer.length)));
  };

  sel.forEach((p, i) => {
    const page = doc.pages.get(p.page);
    if (!page?.parsed) {
      take(page ? page.start : doc.md.length);
      plain(p);
      return;
    }
    const first = i === 0;
    const last = i === sel.length - 1;
    const t = p.layer === null ? null : textSkeleton(p.layer);
    const pm = (first || last) && t ? pageMap(doc, p.page, t) : null;
    // An end before the first char or after the last is the page's own edge.
    const tLen = t ? t.text.length : 0;
    const ts = t && p.start !== null ? charsBefore(t, p.start) : 0;
    const te = t && p.end !== null ? charsBefore(t, p.end) : tLen;

    if (first) {
      if (ts === 0) cursor = pageStart(doc, page, pm);
      else if (ts >= tLen) cursor = pageEnd(doc, page, pm);
      else if (pm) cursor = startAt(doc, toDoc(pm, ts));
      else {
        plain(p);
        return;
      }
    } else if (cursor === null) {
      cursor = page.start;
    }
    if (!last) return;
    const end =
      te >= tLen ? pageEnd(doc, page, pm) : te === 0 ? pageStart(doc, page, pm) : pm ? endAt(doc, toDoc(pm, te)) : null;
    // Unaligned, or carried before the start (text MinerU put elsewhere).
    if (end === null || (cursor !== null && end <= cursor)) {
      if (!first) take(page.start);
      cursor = null;
      plain(p);
      return;
    }
    take(end);
  });
  if (!parsed) return "";
  return normalizeMath(parts.filter(Boolean).join("\n\n"))
    .replace(/\n{3,}/g, "\n\n")
    .trim();
}

// ── DOM edge ─────────────────────────────────────────────────────────────────

/** A text layer's text: its spans' text, a `<br>` as a newline. */
function layerText(layer: Element): { raw: string; nodes: Text[]; starts: number[] } {
  const walker = layer.ownerDocument.createTreeWalker(layer, NodeFilter.SHOW_TEXT | NodeFilter.SHOW_ELEMENT);
  let raw = "";
  const nodes: Text[] = [];
  const starts: number[] = [];
  for (let n = walker.nextNode(); n; n = walker.nextNode()) {
    if (n.nodeType === Node.TEXT_NODE) {
      nodes.push(n as Text);
      starts.push(raw.length);
      raw += (n as Text).data;
    } else if ((n as Element).tagName === "BR") {
      raw += "\n";
    }
  }
  return { raw, nodes, starts };
}

function intersects(node: Node, range: Range): boolean {
  try {
    return range.intersectsNode(node);
  } catch {
    return false;
  }
}

/** Whether `range` covers all of `el`. */
function covers(range: Range, el: Element): boolean {
  try {
    return range.comparePoint(el, 0) === 0 && range.comparePoint(el, el.childNodes.length) === 0;
  } catch {
    return false;
  }
}

/** MinerU's HTML tables (`render.rs`, a table with no crop) as pipe tables. */
function pipeTables(md: string): string {
  if (!/<table[\s>]/i.test(md)) return md;
  return md.replace(/<table[\s>][\s\S]*?<\/table>/gi, (html) => {
    const table = new DOMParser().parseFromString(html, "text/html").querySelector("table");
    const pipe = table ? elementMarkdown(table) : "";
    return pipe ? `\n\n${pipe}\n\n` : html;
  });
}

const FIELD = "input, textarea, [contenteditable='true']";

/** The selection inside `root` (the pdf.js container) as markdown, or "" to
 *  leave the browser's own copy alone. */
export function pdfSelectionMarkdown(
  selection: Selection | null,
  root: HTMLElement,
  pages: ReadonlyMap<number, string>,
  resolveImage: (src: string) => string,
): string {
  if (!selection || selection.rangeCount === 0 || selection.isCollapsed) return "";
  if (root.ownerDocument.activeElement?.closest(FIELD)) return "";
  const range = selection.getRangeAt(0);
  if (!root.contains(range.commonAncestorContainer)) return "";

  const sel: PageSelection[] = [];
  for (const page of Array.from(root.querySelectorAll(".page[data-page-number]"))) {
    if (!intersects(page, range)) continue;
    const n = Number(page.getAttribute("data-page-number"));
    const layer = page.querySelector(".textLayer");
    const text = layer ? layerText(layer) : null;
    if (covers(range, page)) {
      sel.push({ page: n, layer: text?.raw ?? null, start: null, end: null });
      continue;
    }
    if (!text) continue;
    let start = -1;
    let end = -1;
    text.nodes.forEach((node, k) => {
      if (!intersects(node, range)) return;
      const from = text.starts[k] + (node === range.startContainer ? range.startOffset : 0);
      const to = text.starts[k] + (node === range.endContainer ? range.endOffset : node.data.length);
      if (start < 0) start = from;
      end = to;
    });
    if (start >= 0) sel.push({ page: n, layer: text.raw, start, end });
  }
  if (!sel.length || !pages.size) return "";
  const md = markdownOfSelection(parsedDoc(pages), sel, resolveImage);
  return md && pipeTables(md).replace(/\n{3,}/g, "\n\n").trim();
}

function markdownFor(
  target: EventTarget | null,
  root: HTMLElement,
  pages: ReadonlyMap<number, string>,
  resolveImage: (src: string) => string,
): string {
  if (target instanceof Element && target.closest(FIELD)) return "";
  return pdfSelectionMarkdown(window.getSelection(), root, pages, resolveImage);
}

/** Bind as `onCopyCapture`: pdf.js's text layer has its own `copy` listener
 *  that writes the raw text and stops propagation, so a bubbling `onCopy`
 *  never runs. Stopping here keeps pdf.js from overwriting ours. */
export function copyPdfAsMarkdown(
  e: ClipboardEvent,
  root: HTMLElement,
  pages: ReadonlyMap<number, string>,
  resolveImage: (src: string) => string,
): void {
  const md = markdownFor(e.target, root, pages, resolveImage);
  if (!md) return;
  e.clipboardData.setData("text/plain", md);
  e.preventDefault();
  e.stopPropagation();
}

/** The same, dragged out. No `preventDefault` — see docs/ui.md (WebKit drag). */
export function dragPdfAsMarkdown(
  e: DragEvent,
  root: HTMLElement,
  pages: ReadonlyMap<number, string>,
  resolveImage: (src: string) => string,
): void {
  const md = markdownFor(e.target, root, pages, resolveImage);
  if (md) e.dataTransfer.setData("text/plain", md);
}

/** Word characters: letters with their combining marks, digits, underscore. */
const WORD = "\\p{L}\\p{M}\\p{N}_";
export const WORD_RUNS = new RegExp(`[${WORD}]+`, "gu");
const WORD_CHAR = new RegExp(`^[${WORD}]$`, "u");

/** An emoji with its modifiers and ZWJ-joined parts, or a flag. */
const EMOJI =
  "\\p{Extended_Pictographic}(?:[\\p{Emoji_Modifier}\\uFE0F]|\\u200D\\p{Extended_Pictographic})*" +
  "|\\p{Regional_Indicator}{2}";

/** A word run, a run of spaces/tabs, one newline (CRLF counts as one), an
 *  emoji, or any other single code point with its marks — markdown and LaTeX
 *  punctuation stand alone. */
const TOKEN = new RegExp(`[${WORD}]+|[^\\S\\r\\n]+|\\r\\n|${EMOJI}|[\\s\\S]\\p{M}*`, "uy");

/** Characters that belong to the one before them. */
const CLINGS = /^[\p{M}\p{Emoji_Modifier}\uFE0F\u200D]/u;

/** Scripts written without spaces, where a word run is a whole phrase. */
const UNSPACED = /[\p{sc=Thai}\p{sc=Lao}\p{sc=Khmer}\p{sc=Myanmar}\p{sc=Han}\p{sc=Hiragana}\p{sc=Katakana}]/u;

interface WordSegmenter {
  segment(text: string): Iterable<{ index: number }>;
}
type SegmenterCtor = new (locale: undefined, options: { granularity: "word" }) => WordSegmenter;

let segmenter: WordSegmenter | null | undefined;
function wordSegmenter(): WordSegmenter | null {
  if (segmenter === undefined) {
    const Ctor = (Intl as unknown as { Segmenter?: SegmenterCtor }).Segmenter;
    segmenter = Ctor ? new Ctor(undefined, { granularity: "word" }) : null;
  }
  return segmenter;
}

/** Token boundaries of `s[from,to)`: every token's start, then `to`. */
export function tokenBounds(s: string, from: number, to: number): number[] {
  const out: number[] = [];
  const text = from === 0 && to === s.length ? s : s.slice(from, to);
  TOKEN.lastIndex = 0;
  while (TOKEN.lastIndex < text.length) {
    const start = TOKEN.lastIndex;
    const m = TOKEN.exec(text);
    if (!m) break;
    out.push(from + start);
    const seg = m[0].length > 1 && UNSPACED.test(m[0]) ? wordSegmenter() : null;
    if (seg) {
      for (const part of seg.segment(m[0])) if (part.index > 0) out.push(from + start + part.index);
    }
  }
  out.push(to);
  return out;
}

/** Line boundaries of `s[from,to)`: each line keeps its newline. */
export function lineBounds(s: string, from: number, to: number): number[] {
  if (from === to) return [from];
  const out = [from];
  for (let i = s.indexOf("\n", from); i !== -1 && i < to - 1; i = s.indexOf("\n", i + 1)) out.push(i + 1);
  out.push(to);
  return out;
}

const isHigh = (c: number) => c >= 0xd800 && c <= 0xdbff;
const isLow = (c: number) => c >= 0xdc00 && c <= 0xdfff;

/** Is the code point starting at `i` a word character? */
export function wordAt(s: string, i: number): boolean {
  const c = s.codePointAt(i);
  return c !== undefined && !isLow(c) && WORD_CHAR.test(String.fromCodePoint(c));
}

/** Is the code point ending at `i` a word character? */
export function wordBefore(s: string, i: number): boolean {
  if (i <= 0) return false;
  const lo = s.charCodeAt(i - 1);
  return isLow(lo) && i >= 2 && isHigh(s.charCodeAt(i - 2)) ? wordAt(s, i - 2) : wordAt(s, i - 1);
}

/** Would a cut at `i` split a surrogate pair, a CRLF, a letter from its
 *  marks or an emoji from its modifiers? */
export function splitsUnit(s: string, i: number): boolean {
  if (i <= 0 || i >= s.length) return false;
  const before = s.charCodeAt(i - 1);
  const after = s.charCodeAt(i);
  if ((isHigh(before) && isLow(after)) || (before === 13 && after === 10) || before === 0x200d) return true;
  return after >= 0x300 && CLINGS.test(s.slice(i, i + 2));
}

export function countWords(s: string): number {
  let n = 0;
  WORD_RUNS.lastIndex = 0;
  while (WORD_RUNS.exec(s)) n++;
  return n;
}

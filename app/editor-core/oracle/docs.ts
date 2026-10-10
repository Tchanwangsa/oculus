// Random document generators shared by the oracle scripts: sources that mix
// ASCII, Thai, combining marks, astral characters and every line break, and
// code-point-boundary positions in a CodeMirror `Text`.

import { Text } from "@codemirror/state";

import type { Rng } from "./driver";

export const SPLIT = /\r\n?|\n/; // CodeMirror's DefaultSplit

export const RUNS = [
  "a", "z", " ", "word ", "ก", "ไ", "ที่", "ั", "é", "é", "😀", "𝒳", "👩‍👧", "中",
  "$x$", "#", "*",
];
export const BREAKS = ["\n", "\r\n", "\r"];

/** A raw source of about `units` UTF-16 units; `breakRate` is per piece. */
export function source(rng: Rng, units: number, breakRate: number): string {
  const parts: string[] = [];
  let len = 0;
  while (len < units) {
    const piece = rng.chance(breakRate) ? rng.pick(BREAKS) : rng.pick(RUNS);
    parts.push(piece);
    len += piece.length;
  }
  return parts.join("");
}

export const textOf = (s: string) => Text.of(s.split(SPLIT));

export const isLowSurrogate = (doc: Text, pos: number) => {
  if (pos <= 0 || pos >= doc.length) return false;
  const c = doc.sliceString(pos, pos + 1).charCodeAt(0);
  return c >= 0xdc00 && c <= 0xdfff;
};

/** A code-point boundary, uniform over units. */
export function boundary(rng: Rng, doc: Text): number {
  const pos = rng.int(doc.length + 1);
  return isLowSurrogate(doc, pos) ? pos - 1 : pos;
}

/** A boundary range starting near a random point, at most `span` units long. */
export function range(rng: Rng, doc: Text, span: number): [number, number] {
  const from = boundary(rng, doc);
  let to = Math.min(doc.length, from + rng.int(span + 1));
  if (isLowSurrogate(doc, to)) to--;
  return [from, to];
}

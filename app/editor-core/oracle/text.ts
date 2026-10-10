// Checks the Rust `Text` against @codemirror/state's `Text`. Each case is one
// random document (90% tiny, 9.5% 1–30 KB, 0.5% 30–400 KB, so the rope builds
// multi-level trees) checked twice: a `text` probe (line, lineAt, sliceString,
// iter, iterRange, iterLines) and a `text_edits` run of random replace / append /
// slice steps probed after each step, ending with `eq` against the same or a
// same-summary different document. Non-tiny documents are also probed around
// every Rust leaf boundary (asked from the oracle first). Documents mix ASCII,
// Thai, combining marks, astral characters and every line break. The app's
// notes are added when the data directory has them.
//
//   bun editor-core/oracle/text.ts [cases] [seed] [only]     (from app/)

import { Text } from "@codemirror/state";

import { boundary, isLowSurrogate, range, source, textOf } from "./docs";
import { Checker, type Rng, buildOracle, caseRng, corpus, parseArgs, query } from "./driver";

const LINE_TEXT_LIMIT = 2000; // src/bin/oracle/text.rs

const args = parseArgs(2000);
const script = "bun editor-core/oracle/text.ts";

function randomSource(rng: Rng): { source: string; size: "tiny" | "medium" | "large" } {
  // Some documents are one long line, so lines span many leaves.
  const breakRate = rng.pick([0, 0.0005, 0.02, 0.15, 0.4]);
  const r = rng.next();
  if (r < 0.9) return { source: source(rng, rng.int(120), breakRate || 0.1), size: "tiny" };
  if (r < 0.995) return { source: source(rng, rng.logInt(1000, 30000), breakRate), size: "medium" };
  return { source: source(rng, rng.logInt(30000, 400000), breakRate), size: "large" };
}

function boundaries(s: string): number[] {
  const out = [0];
  let pos = 0;
  for (const ch of s) out.push((pos += ch.length));
  return out;
}

interface Queries {
  lines: number[];
  positions: number[];
  ranges: [number, number][];
}

/** FNV-1a (32-bit) over UTF-16 units, as src/bin/oracle/text.rs. */
function fnv1a(s: string): number {
  let h = 0x811c9dc5;
  for (let i = 0; i < s.length; i++) h = Math.imul(h ^ s.charCodeAt(i), 0x01000193);
  return h >>> 0;
}

/** A line as `[number, from, to, text]`; long lines' hashes are memoised by
 * line number in `hashes` (one per document version). */
function lineJson(line: { number: number; from: number; to: number; text: string }, hashes: Map<number, number>) {
  const len = line.to - line.from;
  if (len <= LINE_TEXT_LIMIT) return [line.number, line.from, line.to, line.text];
  let hash = hashes.get(line.number);
  if (hash === undefined) hashes.set(line.number, (hash = fnv1a(line.text)));
  return [line.number, line.from, line.to, { len, fnv1a: hash }];
}

function answer(doc: Text, q: Queries) {
  const hashes = new Map<number, number>();
  return {
    length: doc.length,
    lines: doc.lines,
    line: q.lines.map((n) => lineJson(doc.line(n), hashes)),
    line_at: q.positions.map((pos) => lineJson(doc.lineAt(pos), hashes)),
    slice: q.ranges.map(([from, to]) => doc.sliceString(from, to)),
  };
}

/** The code-point boundaries within two units of each Rust leaf boundary. */
function nearCuts(doc: Text, cuts: number[]): number[] {
  const out = new Set<number>();
  for (const cut of cuts) {
    for (let pos = Math.max(0, cut - 2); pos <= Math.min(doc.length, cut + 2); pos++) {
      if (!isLowSurrogate(doc, pos)) out.add(pos);
    }
  }
  return [...out];
}

/** The `text` probe for one document. Small documents get every line and
 * every boundary; large ones a sample plus every position near a leaf cut. */
function probe(rng: Rng, raw: string, cuts: number[]) {
  const doc = textOf(raw);
  const small = doc.length <= 4000;
  const lines =
    doc.lines <= 2000
      ? Array.from({ length: doc.lines }, (_, i) => i + 1)
      : Array.from({ length: 300 }, () => 1 + rng.int(doc.lines));
  const near = nearCuts(doc, cuts);
  const positions = small
    ? boundaries(doc.toString())
    : [0, doc.length, ...near, ...Array.from({ length: 300 }, () => boundary(rng, doc))];
  const span = small ? doc.length : 3000;
  const ranges = Array.from({ length: 12 }, () => range(rng, doc, span));
  ranges.push(small ? [0, doc.length] : range(rng, doc, 50000));
  // Slices that start, end or straddle at a leaf cut, and span two cuts.
  for (let i = 0; i + 1 < near.length; i += 2) ranges.push([Math.min(near[i], near[i + 1]), Math.max(near[i], near[i + 1])]);
  for (let i = 0; i + 1 < cuts.length; i += 3) ranges.push([cuts[i], cuts[i + 1]]);
  const iter = doc.length <= 40000;
  const iterRanges = Array.from({ length: 4 }, () => {
    const [a, b] = range(rng, doc, small ? doc.length : 5000);
    return (rng.chance(0.5) ? [a, b] : [b, a]) as [number, number];
  });
  const iterLines = Array.from({ length: 4 }, () => {
    const from = 1 + rng.int(doc.lines);
    const to = rng.chance(0.2) ? from : rng.chance(0.1) ? rng.int(doc.lines + 2) : Math.min(doc.lines + 1, from + rng.int(50));
    return [from, to] as [number, number];
  });

  const queries: Queries = { lines, positions, ranges };
  const expected: Record<string, unknown> = answer(doc, queries);
  if (iter) {
    const forward: string[] = [];
    for (const it = doc.iter(); !it.next().done; ) forward.push(it.value);
    const back: string[] = [];
    for (const it = doc.iter(-1); !it.next().done; ) back.push(it.value);
    expected.iter = [forward.join(""), back.reverse().join("")];
  }
  expected.iter_range = iterRanges.map(([from, to]) => {
    const runs: string[] = [];
    for (const it = doc.iterRange(from, to); !it.next().done; ) runs.push(it.value);
    return (from > to ? runs.reverse() : runs).join("");
  });
  expected.iter_lines = iterLines.map(([from, to]) => {
    const out: string[] = [];
    for (const it = doc.iterLines(from, to); !it.next().done; ) out.push(it.value);
    return out;
  });
  const request = { op: "text", doc: raw, ...queries, iter, iter_ranges: iterRanges, iter_lines: iterLines };
  return { request, expected };
}

/** A random insert: mostly a few pieces, sometimes several leaves' worth. */
function insertText(rng: Rng): string {
  if (rng.chance(0.08)) return source(rng, rng.logInt(500, 8000), rng.pick([0, 0.05, 0.3]));
  return source(rng, rng.int(16), 0.15);
}

/** A `text_edits` run: random steps, each followed by queries near the edit. */
function edits(rng: Rng, raw: string, steps: number, finalText: boolean) {
  let doc = textOf(raw);
  const requestSteps: object[] = [];
  const expectedSteps: object[] = [];
  for (let i = 0; i < steps; i++) {
    const kind = doc.length > 0 && rng.chance(0.1) ? "slice" : rng.chance(0.1) ? "append" : "replace";
    let step: Record<string, unknown>;
    let at: number;
    let inserted = 0;
    if (kind === "replace") {
      const big = rng.chance(0.05);
      const [from, to] = range(rng, doc, big ? Math.ceil(doc.length / 5) : rng.pick([0, 0, 2, 10, 60]));
      const insert = insertText(rng);
      const ins = textOf(insert);
      doc = doc.replace(from, to, ins);
      step = { kind, from, to, insert };
      at = from;
      inserted = ins.length;
    } else if (kind === "append") {
      const insert = insertText(rng);
      at = doc.length;
      const ins = textOf(insert);
      doc = doc.append(ins);
      step = { kind, insert };
      inserted = ins.length;
    } else {
      // Keep most of the document so runs don't collapse to nothing.
      let from = rng.int(Math.ceil(doc.length / 10) + 1);
      if (isLowSurrogate(doc, from)) from--;
      let to = doc.length - rng.int(Math.ceil(doc.length / 10) + 1);
      if (isLowSurrogate(doc, to)) to--;
      if (to < from) [from, to] = [to, from];
      doc = doc.slice(from, to);
      step = { kind, from, to };
      at = 0;
    }
    let after = Math.min(doc.length, at + inserted);
    if (isLowSurrogate(doc, after)) after--;
    const lo = Math.max(0, at - 30);
    let hi = Math.min(doc.length, after + 30);
    if (isLowSurrogate(doc, hi)) hi--;
    const queries: Queries = {
      lines: [doc.lineAt(at).number, 1 + rng.int(doc.lines), doc.lines],
      positions: [at, after, boundary(rng, doc), boundary(rng, doc)],
      ranges: [[isLowSurrogate(doc, lo) ? lo - 1 : lo, hi], range(rng, doc, 200), range(rng, doc, 2000)],
    };
    requestSteps.push({ ...step, ...queries });
    expectedSteps.push(answer(doc, queries));
  }
  const request: Record<string, unknown> = { op: "text_edits", doc: raw, steps: requestSteps, final_text: finalText };
  const expected: Record<string, unknown> = { steps: expectedSteps, eq_fresh: true };
  if (finalText) {
    const text = doc.toString();
    expected.text = text;
    // Usually a copy with two code points swapped: same length, lines and
    // bytes, so only a content comparison can say they differ.
    let other = text;
    if (rng.chance(0.75)) {
      const chars = Array.from(text);
      if (chars.length > 1) {
        const [i, j] = [rng.int(chars.length), rng.int(chars.length)];
        [chars[i], chars[j]] = [chars[j], chars[i]];
        other = chars.join("");
      }
    }
    request.eq_other = other;
    expected.eq_other = doc.eq(textOf(other));
  }
  return { request, expected };
}

const binary = buildOracle();
const checker = new Checker("text oracle", binary, args.seed);
const sizes = { tiny: 0, medium: 0, large: 0 };
let stepCount = 0;
let eqFalse = 0;

/** Leaf boundaries for each source; tiny documents are one leaf. */
function leafCuts(sources: string[]): number[][] {
  return query(binary, sources.map((doc) => ({ op: "text_chunks", doc }))).map((a) => a.boundaries);
}

const BLOCK = 2000;
for (let start = 0; start < args.cases; start += BLOCK) {
  const block: { i: number; rng: Rng; raw: string; size: keyof typeof sizes; cuts: number[] }[] = [];
  for (let i = start; i < Math.min(args.cases, start + BLOCK); i++) {
    if (args.only !== null && i !== args.only) continue;
    const rng = caseRng(args.seed, i);
    const { source: raw, size } = randomSource(rng);
    block.push({ i, rng, raw, size, cuts: [] });
  }
  const big = block.filter((c) => c.size !== "tiny");
  leafCuts(big.map((c) => c.raw)).forEach((cuts, k) => (big[k].cuts = cuts));
  for (const { i, rng, raw, size, cuts } of block) {
    sizes[size]++;
    const replay = `${script} ${args.cases} ${args.seed} ${i}`;
    const p = probe(rng, raw, cuts);
    checker.add(`random #${i} (${size}, probe)`, replay, p.request, p.expected);
    const steps = size === "tiny" ? rng.int(6) : 5 + rng.int(36);
    stepCount += steps;
    const e = edits(rng, raw, steps, size !== "large" || rng.chance(0.25));
    if (e.expected.eq_other === false) eqFalse++;
    checker.add(`random #${i} (${size}, edits)`, replay, e.request, e.expected);
  }
}

let notes = 0;
if (args.only === null) {
  const all = await corpus();
  const cuts = leafCuts(all.map((note) => note.source));
  for (const [j, note] of all.entries()) {
    notes++;
    const rng = caseRng(args.seed, 1e9 + j);
    const p = probe(rng, note.source, cuts[j]);
    checker.add(`corpus ${note.name} (probe)`, `${script} ${args.cases} ${args.seed}`, p.request, p.expected);
    const e = edits(rng, note.source, 40, true);
    stepCount += 40;
    checker.add(`corpus ${note.name} (edits)`, `${script} ${args.cases} ${args.seed}`, e.request, e.expected);
  }
}

const randomCases = sizes.tiny + sizes.medium + sizes.large;
checker.finish(
  `${randomCases} random documents (${sizes.tiny} tiny, ${sizes.medium} medium, ${sizes.large} large) + ` +
    `${notes} corpus notes, ${stepCount} edit steps, ${eqFalse} unequal eq checks`,
  randomCases,
);

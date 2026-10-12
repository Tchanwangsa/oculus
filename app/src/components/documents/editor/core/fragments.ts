import { type Input, TreeFragment } from "@lezer/common";

/**
 * `fragments` cut back by one line at each side that touches a change, where
 * @lezer/markdown's block reuse looks too little past the edit:
 *
 * - **After a change** Lezer reuses a block at a line start the fragment
 *   covers, though the change may have rewritten that line's indentation
 *   (`    a` losing a space keeps its `CodeBlock`). The fragment starts at the
 *   next line instead.
 * - **Before a change** Lezer keeps blocks that end before the changed line's
 *   previous line, but a paragraph's end can hang on the line after next (a
 *   table header interrupts it only over a delimiter row). The fragment ends
 *   a line earlier.
 *
 * Mirrors the core's `incremental.rs`, which restarts two lines before a
 * change and resyncs only at a later line start.
 */
export function lineSafeFragments(input: Input, fragments: readonly TreeFragment[]): readonly TreeFragment[] {
  if (!fragments.some((f) => f.openStart || f.openEnd)) return fragments;
  const out: TreeFragment[] = [];
  for (const f of fragments) {
    const from = f.openStart ? nextLineStart(input, f.from) : f.from;
    const to = f.openEnd ? lineStart(input, lineStart(input, f.to) - 1) : f.to;
    if (to <= from) continue;
    out.push(from === f.from && to === f.to ? f : new TreeFragment(from, to, f.tree, f.offset, f.openStart, f.openEnd));
  }
  return out;
}

const CHUNK = 256;

/** The start of the line holding `pos` (`pos` itself at a line start). */
function lineStart(input: Input, pos: number): number {
  while (pos > 0) {
    const from = Math.max(0, pos - CHUNK);
    const at = input.read(from, pos).lastIndexOf("\n");
    if (at >= 0) return from + at + 1;
    pos = from;
  }
  return 0;
}

/** The start of the line after the one holding `pos`, or the input's end. */
function nextLineStart(input: Input, pos: number): number {
  while (pos < input.length) {
    const to = Math.min(input.length, pos + CHUNK);
    const at = input.read(pos, to).indexOf("\n");
    if (at >= 0) return pos + at + 1;
    pos = to;
  }
  return input.length;
}

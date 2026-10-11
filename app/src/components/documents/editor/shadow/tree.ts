import { IterMode, type Tree } from "@lezer/common";

/** Where a Lezer tree and the shadow's `tree()` triples first disagree. */
export interface TreeDifference {
  /** The node's index in pre-order. */
  node: number;
  expected: string;
  actual: string;
}

/** Walks `tree` in pre-order with nested code languages pruned, as the
 *  markdown oracle compares it, against `triples` (`type id, from, to`),
 *  without building either side's list. */
export function treeDifference(tree: Tree, triples: Uint32Array, names: readonly string[]): TreeDifference | null {
  const cursor = tree.cursor(IterMode.IgnoreMounts);
  const rust = (i: number) =>
    i < triples.length ? `${names[triples[i]] ?? `#${triples[i]}`} ${triples[i + 1]} ${triples[i + 2]}` : "(end)";
  let i = 0;
  for (;;) {
    const lezer = `${cursor.name} ${cursor.from} ${cursor.to}`;
    const ok =
      i < triples.length &&
      names[triples[i]] === cursor.name &&
      triples[i + 1] === cursor.from &&
      triples[i + 2] === cursor.to;
    if (!ok) return { node: i / 3, expected: lezer, actual: rust(i) };
    i += 3;
    if (!cursor.next()) break;
  }
  return i < triples.length ? { node: i / 3, expected: "(end)", actual: rust(i) } : null;
}

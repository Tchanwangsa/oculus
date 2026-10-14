import { describe, expect, test } from "bun:test";

import { foldUnchanged, lineDiff, type DiffLine } from "@/lib/ed/lineDiff";

const show = (lines: Array<{ kind: string; text?: string; count?: number }>) =>
  lines.map((l) =>
    l.kind === "fold" ? `…${l.count}` : `${{ same: " ", removed: "-", added: "+" }[l.kind]}${l.text}`,
  );

describe("line diff", () => {
  test("equal texts are all the same", () => {
    expect(show(lineDiff("a\nb", "a\nb"))).toEqual([" a", " b"]);
    expect(lineDiff("", "")).toEqual([]);
  });

  test("one side empty is all removed or all added", () => {
    expect(show(lineDiff("a\nb", ""))).toEqual(["-a", "-b"]);
    expect(show(lineDiff("", "a"))).toEqual(["+a"]);
  });

  test("a changed line is removed then added, between what both keep", () => {
    expect(show(lineDiff("a\nb\nc", "a\nB\nc"))).toEqual([" a", "-b", "+B", " c"]);
  });

  test("keeps the longest common run of lines", () => {
    expect(show(lineDiff("x\na\nb\nc\ny", "a\nb\nz\nc"))).toEqual(["-x", " a", " b", "+z", " c", "-y"]);
  });

  test("CRLF lines match LF lines", () => {
    expect(show(lineDiff("a\r\nb", "a\nb"))).toEqual([" a", " b"]);
  });

  test("a missing final newline shows as an empty last line", () => {
    expect(show(lineDiff("a\n", "a"))).toEqual([" a", "-"]);
  });
});

describe("folding unchanged lines", () => {
  const same = (n: number, from = 0): DiffLine[] =>
    Array.from({ length: n }, (_, i) => ({ kind: "same", text: String(from + i) }));

  test("keeps context beside a change and folds the rest", () => {
    const lines: DiffLine[] = [...same(10), { kind: "added", text: "new" }, ...same(10, 10)];
    expect(show(foldUnchanged(lines, 2))).toEqual(["…8", " 8", " 9", "+new", " 10", " 11", "…8"]);
  });

  test("a run between changes keeps context on both sides", () => {
    const lines: DiffLine[] = [
      { kind: "removed", text: "a" },
      ...same(7),
      { kind: "added", text: "b" },
    ];
    expect(show(foldUnchanged(lines, 2))).toEqual(["-a", " 0", " 1", "…3", " 5", " 6", "+b"]);
  });

  test("never folds a single line", () => {
    const lines: DiffLine[] = [{ kind: "removed", text: "a" }, ...same(5), { kind: "added", text: "b" }];
    expect(show(foldUnchanged(lines, 2))).toEqual(["-a", " 0", " 1", " 2", " 3", " 4", "+b"]);
  });

  test("no change at all folds to one run", () => {
    expect(show(foldUnchanged(same(4), 3))).toEqual(["…4"]);
  });
});

import { describe, expect, test } from "bun:test";
import { followPath, followRoute, rekeyByPath, type PathChange } from "@/lib/shell/pathFollow";
import { filePagePath } from "@/lib/files/labels";

const ids: Record<string, number> = { COMP1: 1, MAST2: 2 };
const idOf = (folder: string) => ids[folder];
const moved = (from: string, to: string): PathChange => ({ moved: [{ from, to }], removed: [] });
const removed = (...paths: string[]): PathChange => ({ moved: [], removed: paths });

describe("followPath", () => {
  test("a moved file takes its new path", () => {
    expect(followPath("courses/COMP1/a.pdf", moved("courses/COMP1/a.pdf", "courses/COMP1/Notes/a.pdf")))
      .toEqual({ kind: "moved", path: "courses/COMP1/Notes/a.pdf" });
  });

  test("a folder's move carries the paths under it, at a / boundary only", () => {
    const change = moved("courses/COMP1/a/b", "courses/COMP1/x");
    expect(followPath("courses/COMP1/a/b/c.pdf", change)).toEqual({ kind: "moved", path: "courses/COMP1/x/c.pdf" });
    expect(followPath("courses/COMP1/a/bc/c.pdf", change)).toEqual({ kind: "same" });
  });

  test("the longest moved prefix wins", () => {
    const change: PathChange = {
      moved: [{ from: "courses/COMP1/a", to: "courses/COMP1/z" }, { from: "courses/COMP1/a/b", to: "courses/COMP1/y" }],
      removed: [],
    };
    expect(followPath("courses/COMP1/a/b/c", change)).toEqual({ kind: "moved", path: "courses/COMP1/y/c" });
  });

  test("a deleted folder takes the paths under it", () => {
    expect(followPath("courses/COMP1/a/c.pdf", removed("courses/COMP1/a"))).toEqual({ kind: "removed" });
    expect(followPath("courses/COMP1/ab/c.pdf", removed("courses/COMP1/a"))).toEqual({ kind: "same" });
  });
});

describe("followRoute on a file page", () => {
  const page = (id: number, rel: string) => filePagePath(id, rel);

  test("a rename follows, keeping the subject", () => {
    expect(followRoute(page(1, "courses/COMP1/old.md"), moved("courses/COMP1/old.md", "courses/COMP1/new.md"), idOf))
      .toEqual({ kind: "moved", path: page(1, "courses/COMP1/new.md") });
  });

  test("a move between subjects takes the other subject's id", () => {
    expect(followRoute(page(1, "courses/COMP1/a.pdf"), moved("courses/COMP1/a.pdf", "courses/MAST2/a.pdf"), idOf))
      .toEqual({ kind: "moved", path: page(2, "courses/MAST2/a.pdf") });
  });

  test("a file in a moved folder follows it, with names that need encoding", () => {
    const route = page(1, "courses/COMP1/Week 1/a #2?.pdf");
    expect(followRoute(route, moved("courses/COMP1/Week 1", "courses/COMP1/Wk 1"), idOf))
      .toEqual({ kind: "moved", path: page(1, "courses/COMP1/Wk 1/a #2?.pdf") });
  });

  test("a deleted file falls back to the Files tab on its folder", () => {
    expect(followRoute(page(1, "courses/COMP1/Notes/a.pdf"), removed("courses/COMP1/Notes/a.pdf"), idOf))
      .toEqual({ kind: "removed", fallback: "/subjects/1/files/Notes" });
    expect(followRoute(page(1, "courses/COMP1/a.pdf"), removed("courses/COMP1/a.pdf"), idOf))
      .toEqual({ kind: "removed", fallback: "/subjects/1/files" });
  });

  test("an untouched file and other routes stay", () => {
    expect(followRoute(page(1, "courses/COMP1/b.pdf"), moved("courses/COMP1/a.pdf", "courses/COMP1/c.pdf"), idOf))
      .toEqual({ kind: "same" });
    expect(followRoute("/chat?t=1", removed("courses/COMP1/a"), idOf)).toEqual({ kind: "same" });
  });
});

describe("followRoute on a folder page", () => {
  test("Drive follows a moved folder, encoding each segment", () => {
    expect(followRoute("/drive/COMP1/Week%201/Labs", moved("courses/COMP1/Week 1", "courses/COMP1/A #1"), idOf))
      .toEqual({ kind: "moved", path: "/drive/COMP1/A%20%231/Labs" });
  });

  test("Drive on a deleted folder moves up to what held it", () => {
    expect(followRoute("/drive/COMP1/a/b", removed("courses/COMP1/a"), idOf))
      .toEqual({ kind: "moved", path: "/drive/COMP1" });
  });

  test("the Files tab follows a moved folder, into another subject's tab", () => {
    expect(followRoute("/subjects/1/files/a/b", moved("courses/COMP1/a", "courses/COMP1/z"), idOf))
      .toEqual({ kind: "moved", path: "/subjects/1/files/z/b" });
    expect(followRoute("/subjects/1/files/a", moved("courses/COMP1/a", "courses/MAST2/a"), idOf))
      .toEqual({ kind: "moved", path: "/subjects/2/files/a" });
  });

  test("the Files tab on a deleted folder moves up; another subject's change leaves it", () => {
    expect(followRoute("/subjects/1/files/a/b", removed("courses/COMP1/a"), idOf))
      .toEqual({ kind: "moved", path: "/subjects/1/files" });
    expect(followRoute("/subjects/2/files/a", removed("courses/COMP1/a"), idOf)).toEqual({ kind: "same" });
    expect(followRoute("/subjects/1/files/ab", removed("courses/COMP1/a"), idOf)).toEqual({ kind: "same" });
  });
});

test("rekeyByPath moves and drops keys, keeps what it is told to, and is a no-op otherwise", () => {
  const change: PathChange = { moved: [{ from: "a", to: "b" }], removed: ["c"] };
  const follow = (p: string) => followPath(p, change);
  const record = { a: 1, c: 2, d: 3 };
  expect(rekeyByPath(record, follow)).toEqual({ b: 1, d: 3 });
  expect(rekeyByPath(record, follow, undefined, (v) => v === 1)).toEqual({ a: 1, d: 3 });
  const untouched = { d: 3 };
  expect(rekeyByPath(untouched, follow)).toBe(untouched);
});

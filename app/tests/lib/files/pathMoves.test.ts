import { describe, expect, spyOn, test } from "bun:test";
import type Database from "@tauri-apps/plugin-sql";
import * as connection from "@/lib/db/connection";
import { forgetPathMoves, movedPath, resolveMoved, type PathMove } from "@/lib/files/pathMoves";

const moves = [
  { old_prefix: "courses/X/uploads", new_prefix: "courses/X" },
  { old_prefix: "courses/X/uploads/a.pdf", new_prefix: "courses/X/a (2).pdf" },
  { old_prefix: "courses/X/files", new_prefix: "courses/X/Course/Files" },
  { old_prefix: "lectures/L1", new_prefix: "courses/X/Course/Recordings/2026-03-02-mo-l101" },
];

describe("old library paths", () => {
  test("the longest prefix wins and what follows it carries over", () => {
    expect(movedPath("courses/X/uploads/b.pdf", moves)).toBe("courses/X/b.pdf");
    expect(movedPath("courses/X/uploads/a.pdf#page=3", moves)).toBe("courses/X/a (2).pdf#page=3");
    expect(movedPath("courses/X/files/n.md:L15", moves)).toBe("courses/X/Course/Files/n.md:L15");
    expect(movedPath("lectures/L1/frames/1.jpg", moves)).toBe(
      "courses/X/Course/Recordings/2026-03-02-mo-l101/frames/1.jpg",
    );
  });

  test("only at a path boundary, and never a path already in the new tree", () => {
    expect(movedPath("courses/X/filesystem.md", moves)).toBeNull();
    expect(movedPath("courses/X/Course/Files/a.pdf", moves)).toBeNull();
    expect(movedPath("lectures/L10/source1.mp4", moves)).toBeNull();
  });
});

describe("rules a Drive move adds", () => {
  // `followLibraryChange` forgets the rules on every `drive-changed` move.
  test("are read on the next lookup after a move", async () => {
    const rows: PathMove[] = [{ old_prefix: "courses/X/canvases", new_prefix: "courses/X" }];
    const db = spyOn(connection, "getDb").mockResolvedValue({
      select: async (sql: string) => (sql.includes("path_moves") ? [...rows] : []),
    } as unknown as Database);
    try {
      expect(await resolveMoved("courses/X/canvases/a.canvas")).toBe("courses/X/a.canvas");
      // Drive's reconcile renames the drawing and chains its rule.
      rows.push({ old_prefix: "courses/X/canvases/a.canvas", new_prefix: "courses/X/a.whiteboard" });
      forgetPathMoves();
      expect(await resolveMoved("courses/X/canvases/a.canvas")).toBe("courses/X/a.whiteboard");
    } finally {
      db.mockRestore();
    }
  });
});

import { describe, expect, test } from "bun:test";

import { conflictOriginal, splitListing } from "@/lib/ed/conflicts";

describe("a server copy's file", () => {
  test("drops .ed-server before the last extension or at the end", () => {
    expect(conflictOriginal("tally.ed-server.py")).toBe("tally.py");
    expect(conflictOriginal("src/a.ed-server.py")).toBe("src/a.py");
    expect(conflictOriginal("data.tar.ed-server.gz")).toBe("data.tar.gz");
    expect(conflictOriginal("Makefile.ed-server")).toBe("Makefile");
    expect(conflictOriginal(".env.ed-server")).toBe(".env");
  });

  test("anything else is no server copy", () => {
    expect(conflictOriginal("tally.py")).toBeNull();
    expect(conflictOriginal(".ed-server")).toBeNull();
    expect(conflictOriginal("a.ed-server.d/b.py")).toBeNull();
    expect(conflictOriginal("a.ed-server.tar.gz")).toBeNull();
    expect(conflictOriginal("a.ed-servers.py")).toBeNull();
  });
});

describe("splitting the file list", () => {
  test("server copies beside listed files are conflicts, hidden from the tabs", () => {
    const { files, conflicts } = splitListing([
      "Makefile",
      "Makefile.ed-server",
      "src/a.ed-server.py",
      "src/a.py",
      "tally.py",
    ]);
    expect(files).toEqual(["Makefile", "src/a.py", "tally.py"]);
    expect([...conflicts]).toEqual([
      ["Makefile", "Makefile.ed-server"],
      ["src/a.py", "src/a.ed-server.py"],
    ]);
  });

  test("a copy whose file isn't listed stays a tab", () => {
    const { files, conflicts } = splitListing(["notes.ed-server.txt", "tally.py"]);
    expect(files).toEqual(["notes.ed-server.txt", "tally.py"]);
    expect(conflicts.size).toBe(0);
  });

  test("a conflict event counts while its file is listed, its copy hidden", () => {
    const known = new Map([
      ["tally.py", "tally.ed-server.py"],
      ["gone.py", "gone.ed-server.py"],
    ]);
    const { files, conflicts } = splitListing(["tally.ed-server.py", "tally.py"], known);
    expect(files).toEqual(["tally.py"]);
    expect([...conflicts]).toEqual([["tally.py", "tally.ed-server.py"]]);
    expect(splitListing(["tally.py"], known).files).toEqual(["tally.py"]);
  });
});

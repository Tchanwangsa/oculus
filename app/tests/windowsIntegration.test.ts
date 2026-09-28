import { describe, expect, test } from "bun:test";
import { Database } from "bun:sqlite";
import { fileDropRatio } from "../src/lib/fileDrop";
import { libraryPath } from "../src/lib/libraryPath";
import { shouldDiscoverModels, type ModelDiscoveryAttempt } from "../src/lib/modelDiscovery";
import { TERM_RANK_SQL, termRank } from "../src/lib/terms";

describe("Windows integration with the new upstream UI", () => {
  test("empty CLI catalogues recover after recheck without render loops or overlapping requests", () => {
    const installed = [{ provider: "codex", path: "C:\\bin\\codex.cmd" }];
    const rechecked = [...installed];
    const attempt: ModelDiscoveryAttempt = { health: null, version: 0, status: "pending" };
    expect(shouldDiscoverModels(undefined, null, 0)).toBe(true);
    // Installation or a catalogue edit may finish while the first lookup runs.
    expect(shouldDiscoverModels(attempt, installed, 0)).toBe(false);
    expect(shouldDiscoverModels(attempt, installed, 1)).toBe(false);
    attempt.status = "empty";
    expect(shouldDiscoverModels(attempt, installed, 0)).toBe(true);
    attempt.health = installed;
    // An empty retry is final until another successful health check occurs.
    expect(shouldDiscoverModels(attempt, installed, 0)).toBe(false);
    expect(shouldDiscoverModels(attempt, rechecked, 0)).toBe(true);
    attempt.status = "ready";
    expect(shouldDiscoverModels(attempt, rechecked, 0)).toBe(false);
    expect(shouldDiscoverModels(attempt, rechecked, 1)).toBe(true);
  });

  test("native file drops hit CSS targets at Windows DPI and app zoom", () => {
    // 200% Windows display scale, then 125% app zoom. A physical 500px
    // coordinate must reach CSS 200px, not 400px in an adjacent split pane.
    expect(500 * fileDropRatio(960, 2400, 2, false)).toBe(200);
    // macOS reports the same position already in logical points.
    expect(250 * fileDropRatio(960, 2400, 2, true)).toBe(200);
    expect(fileDropRatio(0, 0, 0, false)).toBe(1);
  });

  test("agent links accept Windows paths, encoded spaces and line citations safely", () => {
    const relative = "courses/TEST/files/lecture 1.pdf.md";
    expect(libraryPath("..\\courses\\TEST\\files\\lecture 1.pdf.md:97-120")).toBe(relative);
    expect(libraryPath("C:\\Users\\student\\AppData\\Roaming\\com.tchan.oculus\\courses\\TEST\\files\\lecture%201.pdf.md:97")).toBe(relative);
    expect(libraryPath("/Users/student/Library/Application%20Support/com.tchan.oculus/" + relative)).toBe(relative);
    for (const bad of ["C:\\Other\\courses\\TEST\\x.pdf", "../courses/TEST/../other.pdf", "courses/TEST/%2e%2e/other.pdf", "courses/TEST/x%0A.pdf", "cat ../courses/TEST/x.pdf"]) {
      expect(libraryPath(bad)).toBeNull();
    }
  });

  test("month intensives and Windows term abbreviations share SQL ordering", () => {
    const db = new Database(":memory:");
    try {
      for (const [name, rank] of [["2026 June", 2], ["2026_JUN", 2], ["2026 January", 0], ["2026_FEB", 0], ["2026 Semester 2 (July intensive)", 3], ["2026_SM1", 1]] as const) {
        expect(termRank(name)).toBe(rank);
        expect(db.query(`SELECT ${TERM_RANK_SQL("$1")} AS rank`).get(name)).toEqual({ rank });
      }
    } finally { db.close(); }
  });
});

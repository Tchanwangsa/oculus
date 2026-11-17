import { describe, expect, test } from "bun:test";
import {
  filterListing,
  isFlat,
  kindOf,
  matchesQuery,
  modifiedSince,
  NO_FILTERS,
  sourceOf,
  type DriveFilters,
} from "@/lib/drive/filters";
import type { FileEntry, FolderEntry } from "@/lib/drive/listing";
import type { DbFile } from "@/lib/db";

const NOW = new Date(2026, 10, 17, 15, 0);
const DAY = 86_400_000;

const file = (relativePath: string, zone: "own" | "synced", modified: number): FileEntry => ({
  relativePath,
  name: relativePath.split("/").pop()!,
  zone,
  editable: zone === "own",
  size: 1,
  modified,
});
const folder = (name: string): FolderEntry => ({
  relativePath: `x/${name}`,
  name,
  zone: "own",
  writable: true,
  editable: true,
  childCount: 0,
  modified: null,
});
const row = (relative_path: string, category: string) => ({ relative_path, category }) as DbFile;

describe("kindOf", () => {
  test("goes by extension", () => {
    expect(kindOf("Lecture 5.PDF")).toBe("pdf");
    expect(kindOf("deck.pptx")).toBe("slides");
    expect(kindOf("marks.xlsx")).toBe("sheet");
    expect(kindOf("essay.docx")).toBe("document");
    expect(kindOf("board.canvas")).toBe("canvas");
    expect(kindOf("paper.notebook")).toBe("notebook");
    expect(kindOf("demo.mov")).toBe("video");
    expect(kindOf("a.png")).toBe("image");
    expect(kindOf("main.py")).toBe("code");
    expect(kindOf("starter.zip")).toBe("archive");
    expect(kindOf("README")).toBe("other");
  });

  test("markdown is a note unless the sync wrote it", () => {
    expect(kindOf("retro.md")).toBe("note");
    expect(kindOf("retro.md", "document")).toBe("note");
    expect(kindOf("week-3.md", "page")).toBe("document");
  });
});

test("sourceOf: the student's zone first, then Ed by category", () => {
  expect(sourceOf("own", "upload")).toBe("yours");
  expect(sourceOf("synced", "ed")).toBe("ed");
  expect(sourceOf("synced", "lessons")).toBe("ed");
  expect(sourceOf("synced", "file")).toBe("canvas");
  expect(sourceOf("synced", undefined)).toBe("canvas");
});

test("modifiedSince counts whole local days back", () => {
  expect(modifiedSince("any", NOW)).toBeNull();
  const today = new Date(2026, 10, 17).getTime();
  expect(modifiedSince("today", NOW)).toBe(today);
  expect(modifiedSince("week", NOW)).toBe(today - 6 * DAY);
  expect(modifiedSince("year", NOW)).toBe(new Date(2026, 0, 1).getTime());
});

test("matchesQuery needs every word, any case, any order", () => {
  expect(matchesQuery("Lecture 5 – Agile at scale.pdf", "agile lecture")).toBe(true);
  expect(matchesQuery("Lecture 5.pdf", "agile")).toBe(false);
  expect(matchesQuery("anything", "  ")).toBe(true);
});

describe("filters → the flat list", () => {
  const listing = {
    folders: [folder("Exam prep"), folder("Week 3")],
    files: [
      file("c/Course/Files/w3.pdf", "synced", NOW.getTime() - 2 * DAY),
      file("c/Course/Discussion/t.md", "synced", NOW.getTime() - 40 * DAY),
      file("c/notes.md", "own", NOW.getTime()),
      file("c/exam.pdf", "own", NOW.getTime() - 400 * DAY),
    ],
  };
  const rows = new Map([
    ["c/Course/Files/w3.pdf", row("c/Course/Files/w3.pdf", "file")],
    ["c/Course/Discussion/t.md", row("c/Course/Discussion/t.md", "ed")],
  ]);
  const names = (f: DriveFilters) => {
    const out = filterListing(listing, rows, f, NOW);
    return [...out.folders.map((d) => d.name), ...out.files.map((x) => x.name)];
  };

  test("no filter is not flat", () => {
    expect(isFlat(NO_FILTERS)).toBe(false);
    expect(isFlat({ ...NO_FILTERS, query: " " })).toBe(false);
    expect(isFlat({ ...NO_FILTERS, query: "w" })).toBe(true);
    expect(isFlat({ ...NO_FILTERS, modified: "week" })).toBe(true);
  });

  test("a search alone keeps matching folders", () => {
    expect(names({ ...NO_FILTERS, query: "exam" })).toEqual(["Exam prep", "exam.pdf"]);
  });

  test("Type, From and Modified drop folders and combine", () => {
    expect(names({ ...NO_FILTERS, kinds: new Set(["pdf"]) })).toEqual(["w3.pdf", "exam.pdf"]);
    expect(names({ ...NO_FILTERS, kinds: new Set(["pdf"]), sources: new Set(["canvas"]) })).toEqual(["w3.pdf"]);
    expect(names({ ...NO_FILTERS, sources: new Set(["ed"]) })).toEqual(["t.md"]);
    expect(names({ ...NO_FILTERS, modified: "week" })).toEqual(["w3.pdf", "notes.md"]);
    expect(names({ ...NO_FILTERS, kinds: new Set(["note"]) })).toEqual(["notes.md"]);
  });
});

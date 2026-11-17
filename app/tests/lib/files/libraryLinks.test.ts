import { describe, expect, test } from "bun:test";
import { libraryLinkTarget } from "@/lib/files/libraryLinks";
import type { DbFile } from "@/lib/db";

describe("inherited note links", () => {
  test("percent-encoded note links decode spaces, parentheses, percent and Unicode", () => {
    const file = { relative_path: "courses/COMP30026_2026_SM2/Exam prep/Notes (week 2) 100% λ.pdf" } as DbFile;
    expect(libraryLinkTarget("../Exam%20prep/Notes%20%28week%202%29%20100%25%20%CE%BB.pdf", [file])).toBe(file);
  });
  test("renamed Canvas pages resolve by canonical source URL and malformed percentages do not throw", () => {
    const file = { relative_path: "courses/X/Course/Pages/new-title.md", source_url: "https://canvas.test/courses/1/pages/original-slug" } as DbFile;
    expect(libraryLinkTarget(file.source_url!, [file])).toBe(file);
    expect(libraryLinkTarget("../Pages/stray%name.md", [])).toBeUndefined();
  });
  test("a ../ link resolves against the linking file's folder before any suffix match", () => {
    const synced = { relative_path: "courses/X/Course/Files/a.pdf" } as DbFile;
    const own = { relative_path: "courses/X/My Files/a.pdf" } as DbFile;
    expect(libraryLinkTarget("../Files/a.pdf", [own, synced], "courses/X/Course/Modules/01-intro.md")).toBe(synced);
    expect(libraryLinkTarget("../Files/a.pdf", [own, synced])).toBe(synced);
  });
});

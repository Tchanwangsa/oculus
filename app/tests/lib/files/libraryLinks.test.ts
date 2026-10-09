import { describe, expect, test } from "bun:test";
import { libraryLinkTarget } from "@/lib/files/libraryLinks";
import type { DbFile } from "@/lib/db";

describe("inherited note links", () => {
  test("percent-encoded note links decode spaces, parentheses, percent and Unicode", () => {
    const file = { relative_path: "courses/COMP30026_2026_SM2/uploads/Notes (week 2) 100% λ.pdf" } as DbFile;
    expect(libraryLinkTarget("../uploads/Notes%20%28week%202%29%20100%25%20%CE%BB.pdf", [file])).toBe(file);
  });
  test("renamed Canvas pages resolve by canonical source URL and malformed percentages do not throw", () => {
    const file = { relative_path: "courses/X/pages/new-title.md", source_url: "https://canvas.test/courses/1/pages/original-slug" } as DbFile;
    expect(libraryLinkTarget(file.source_url!, [file])).toBe(file);
    expect(libraryLinkTarget("../pages/stray%name.md", [])).toBeUndefined();
  });
});

import { describe, expect, test } from "bun:test";
import { splitFrontmatter, stripFrontmatter } from "@/lib/markdown/frontmatter";

const text = [
  "---",
  "ed_lesson_id: 123",
  'title: "Assignment 1: \\"Logic\\""',
  "due_at: null",
  "slides:",
  '  - {"id":1,"title":"Q1","type":"code","status":"completed"}',
  '  - {"id":2,"title":"Notes","type":"pdf","status":"unseen"}',
  'url: "https://example.org/lessons/123"',
  "---",
  "# Assignment 1",
  "",
  "Body with --- inside.",
].join("\n");

describe("scraped front matter", () => {
  test("reads JSON scalars and block lists", () => {
    const { data, body } = splitFrontmatter(text);
    expect(data.ed_lesson_id).toBe(123);
    expect(data.title).toBe('Assignment 1: "Logic"');
    expect(data.due_at).toBeNull();
    expect(data.slides).toEqual([
      { id: 1, title: "Q1", type: "code", status: "completed" },
      { id: 2, title: "Notes", type: "pdf", status: "unseen" },
    ]);
    expect(data.url).toBe("https://example.org/lessons/123");
    expect(body).toBe("# Assignment 1\n\nBody with --- inside.");
  });

  test("leaves markdown without front matter untouched", () => {
    const plain = "# Title\n\n---\n\nafter a rule";
    expect(splitFrontmatter(plain)).toEqual({ data: {}, body: plain });
    expect(stripFrontmatter(text).startsWith("# Assignment 1")).toBe(true);
  });

  test("keeps a page that opens with a rule around plain text", () => {
    for (const page of ["---\nNote: bring a calculator\n---\n\nBody", "---\n\n---\nBody"]) {
      expect(splitFrontmatter(page)).toEqual({ data: {}, body: page });
    }
  });
});

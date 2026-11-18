import { expect, test } from "bun:test";
import { inline, previewFormat, textPreview } from "@/lib/drive/textPreview";

test("markdown drops frontmatter and marks headings, items and quotes", () => {
  const md = [
    "---",
    "title: Week 3",
    "tags: [a, b]",
    "---",
    "# Week 3 notes",
    "",
    "Intro with **bold**, _italic_ and `code`.",
    "",
    "## Tasks",
    "- [ ] read [chapter 2](https://example.com)",
    "  * nested item",
    "1. first",
    "> a quote",
  ].join("\n");
  expect(textPreview(md, "markdown")).toEqual([
    { style: "h1", text: "Week 3 notes" },
    { style: "gap", text: "" },
    { style: "body", text: "Intro with bold, italic and code." },
    { style: "gap", text: "" },
    { style: "h2", text: "Tasks" },
    { style: "bullet", marker: "☐", depth: 0, text: "read chapter 2" },
    { style: "bullet", marker: "•", depth: 1, text: "nested item" },
    { style: "bullet", marker: "1.", depth: 0, text: "first" },
    { style: "quote", text: "a quote" },
  ]);
});

test("fenced code keeps its lines; rules, table separators and comments go", () => {
  const md = "Text\n\n```py\nx = 1\n```\n---\n| a | b |\n|---|:-:|\n| 1 | 2 |\n<!-- hidden\nstill hidden -->\nEnd";
  expect(textPreview(md, "markdown")).toEqual([
    { style: "body", text: "Text" },
    { style: "gap", text: "" },
    { style: "code", text: "x = 1" },
    { style: "body", text: "a   b" },
    { style: "body", text: "1   2" },
    { style: "body", text: "End" },
  ]);
});

test("inline markup is removed but snake_case and stray symbols survive", () => {
  expect(inline("see [[Lecture 4|the lecture]] and ![fig](a.png)")).toBe("see the lecture and fig");
  expect(inline("snake_case_name stays, a < b > c too")).toBe("snake_case_name stays, a < b > c too");
  expect(inline("~~old~~ <b>new</b> \\*literal\\*")).toBe("old new *literal*");
});

test("plain text and code keep their lines, with no gap at either end", () => {
  expect(textPreview("\n\nfn main() {\n\treturn;\n}\n\n", "code")).toEqual([
    { style: "code", text: "fn main() {" },
    { style: "code", text: "  return;" },
    { style: "code", text: "}" },
  ]);
  expect(textPreview("a\r\n\r\n\r\nb", "plain")).toEqual([
    { style: "body", text: "a" },
    { style: "gap", text: "" },
    { style: "body", text: "b" },
  ]);
});

test("a long file previews only its head", () => {
  const lines = textPreview(Array.from({ length: 500 }, (_, i) => `line ${i}`).join("\n"), "plain");
  expect(lines.length).toBe(40);
});

test("format follows the extension, then the code kind", () => {
  expect(previewFormat("notes.md", "note")).toBe("markdown");
  expect(previewFormat("README.txt", "document")).toBe("plain");
  expect(previewFormat("data.json", "code")).toBe("code");
  expect(previewFormat("report.docx", "document")).toBeNull();
  expect(previewFormat("song.mp3", "audio")).toBeNull();
});

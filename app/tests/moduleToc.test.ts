import { describe, expect, test } from "bun:test";
import { parseModuleToc, resolveTocHref } from "../src/lib/moduleToc";
import { isPdfBacked, isVideoFile } from "../src/lib/fileTypes";

// Lines exactly as `file_toc_line` and `scrape_modules` in sync.rs write them.
const TOC = [
  "# Week 1",
  "",
  "- [Slides](../files/w1.pdf)",
  "  - [Matrices \\[Part 1\\]](../files/Matrices_Part_1_-_Zoom_Recording__12_min_1x__8_min_at_1.5x_.mp4) _(video 12345)_",
  "- Locked notes _(file)_",
  "- [Quiz 1](../quizzes/quiz-1.md) _(quiz)_",
  "",
].join("\n");

describe("module TOC", () => {
  const items = parseModuleToc(TOC).sections.flatMap((s) => s.items);

  test("a video item carries its Canvas id and where the download lands", () => {
    const video = items[1];
    expect(video).toMatchObject({
      title: "Matrices [Part 1]",
      kind: "video",
      canvasFileId: 12345,
      indent: 1,
    });
    expect(resolveTocHref(video.href!, "modules/01-week-1.md")).toEqual({
      kind: "internal",
      path: "files/Matrices_Part_1_-_Zoom_Recording__12_min_1x__8_min_at_1.5x_.mp4",
    });
  });

  test("other items keep their kinds and no Canvas id", () => {
    expect(items[0]).toMatchObject({ href: "../files/w1.pdf", kind: null, canvasFileId: null });
    expect(items[2]).toMatchObject({ title: "Locked notes", href: null, kind: "file", canvasFileId: null });
    expect(items[3]).toMatchObject({ kind: "quiz", canvasFileId: null });
  });
});

test("videos are recognised and kept out of the PDF pipeline", () => {
  for (const name of ["a.mp4", "b.MOV", "c.m4v", "d.webm"]) {
    expect(isVideoFile(name)).toBe(true);
    expect(isPdfBacked(name)).toBe(false);
  }
  expect(isVideoFile("slides.pdf")).toBe(false);
  expect(isVideoFile("mp4")).toBe(false);
});

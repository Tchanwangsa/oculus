import { describe, expect, test } from "bun:test";
import { fold, foldedPattern, matchPage, pageCorpus, partRect } from "../src/lib/pdfFind";
import type { PdfLine } from "../src/lib/pdfView";
import { fitScale, layoutPages, shownPages, UNIT } from "../src/components/files/pdf/layout";

/** A horizontal line at `y` whose chars are 10pt wide from x = 0. */
function line(text: string, y = 0): PdfLine {
  return {
    text,
    x: 0,
    y,
    width: text.length * 10,
    height: 12,
    vertical: false,
    chars: Array.from({ length: text.length + 1 }, (_, i) => i * 10),
  };
}

function find(lines: PdfLine[], query: string) {
  return matchPage(1, pageCorpus(lines), foldedPattern(query)!, 100).map((m) => m.parts);
}

describe("PDF find", () => {
  test("folds case, diacritics and ligatures, keeping source offsets", () => {
    const f = fold("Café ﬁn");
    expect(f.text).toBe("cafe fin");
    // "ﬁ" (one source char at 5) becomes two folded chars.
    expect(f.map).toEqual([0, 1, 2, 3, 4, 5, 5, 6]);
  });

  test("matches regardless of case and accents", () => {
    expect(find([line("Résumé of the CAFE")], "resume")).toEqual([[{ line: 0, start: 0, end: 6 }]]);
    expect(find([line("Résumé of the CAFE")], "café")).toEqual([[{ line: 0, start: 14, end: 18 }]]);
  });

  test("a match runs across a line break as two parts", () => {
    expect(find([line("the quick"), line("brown fox", 14)], "quick brown")).toEqual([
      [
        { line: 0, start: 4, end: 9 },
        { line: 1, start: 0, end: 5 },
      ],
    ]);
  });

  test("a ligature in the text maps back to its one character", () => {
    expect(find([line("ﬁnd")], "fin")).toEqual([[{ line: 0, start: 0, end: 2 }]]);
  });

  test("boxes come from the char stops", () => {
    expect(partRect(line("hello"), 1, 3)).toEqual({ x: 10, y: 0, width: 20, height: 12 });
  });
});

describe("PDF layout", () => {
  const portrait = { width: 600, height: 800 };
  const slide = { width: 800, height: 450 };

  test("a portrait page fits the width, a slide fits whole, both capped", () => {
    const s = fitScale([portrait], "scroll", 1, 432 + 32, 300);
    expect(s * UNIT * 600).toBeCloseTo(432);
    const t = fitScale([slide], "scroll", 1, 2000, 300 + 32);
    expect(t * UNIT * 450).toBeCloseTo(300);
    expect(fitScale([portrait], "scroll", 1, 5000, 5000)).toBe(1.25);
  });

  test("continuous scroll stacks pages and names the ones in view", () => {
    const lay = layoutPages([portrait, portrait, portrait], "scroll", 0.75, 1, 1000);
    expect(lay.boxes.map((b) => b.top)).toEqual([16, 832, 1648]);
    expect(shownPages(lay, 0, 600)?.shown).toEqual({ first: 1, last: 1 });
    expect(shownPages(lay, 500, 600)?.shown).toEqual({ first: 1, last: 2 });
  });

  test("a spread pairs odd with the next page", () => {
    const lay = layoutPages([portrait, portrait, portrait], "spread", 1, 3, 400);
    expect(lay.boxes.map((b) => b.page)).toEqual([3]);
    expect(layoutPages([portrait, portrait, portrait], "spread", 1, 2, 400).boxes.map((b) => b.page)).toEqual([1, 2]);
  });
});

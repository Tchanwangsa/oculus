import { describe, expect, test } from "bun:test";
import { normalizeMath } from "@/lib/markdown/math";

describe("normalizeMath", () => {
  test("a line holding one $$ formula becomes a display block", () => {
    expect(normalizeMath("part.\n$$a=b$$\nwhere")).toBe("part.\n\n$$\na=b\n$$\n\nwhere");
  });

  test("$$ inside a sentence, two formulas on a line and code stay as written", () => {
    for (const text of ["x $$a$$ y", "$$a$$ and $$b$$", "`$$e$$`", "$$\nd\n$$"]) {
      expect(normalizeMath(text)).toBe(text);
    }
  });

  test("bracket delimiters still become dollars", () => {
    expect(normalizeMath("\\(x\\) and \\[y\\] here")).toBe("$x$ and $$y$$ here");
  });
});

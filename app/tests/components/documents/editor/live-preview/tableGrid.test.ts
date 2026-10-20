import { describe, expect, test } from "bun:test";

import { gridFromText, gridToHtml, gridToMarkdown, gridToText } from "@/components/documents/editor/live-preview/tableGrid";

describe("gridFromText", () => {
  test("tab-separated rows, quotes stripped", () => {
    expect(gridFromText("a\tb\n1\t\"x\"\"y\"\n", true)).toEqual([["a", "b"], ["1", 'x"y']]);
  });

  test("prose and indented code are not grids when strict", () => {
    expect(gridFromText("just a line", true)).toBeNull();
    expect(gridFromText("one\ttwo", true)).toBeNull();
    expect(gridFromText("\tfoo\n\t\tbar\n\tbaz", true)).toBeNull();
    expect(gridFromText("a\tb\nc", true)).toBeNull();
  });

  test("a single cell or line is a grid only when it has a tab or break, loosely", () => {
    expect(gridFromText("plain", false)).toBeNull();
    expect(gridFromText("a\tb", false)).toEqual([["a", "b"]]);
    expect(gridFromText("a\nb", false)).toEqual([["a"], ["b"]]);
  });
});

describe("gridToMarkdown", () => {
  test("header, delimiter and body, pipes escaped, short rows padded", () => {
    expect(gridToMarkdown([["Path", "Label"], ["00->00", "a|s"], ["x"]])).toBe(
      "| Path | Label |\n| --- | --- |\n| 00->00 | a\\|s |\n| x |  |",
    );
  });

  test("alignment and flattened line breaks", () => {
    expect(gridToMarkdown([["a", "b", "c"], ["1\n2", "", "3"]], ["left", "center", "right"])).toBe(
      "| a | b | c |\n| :--- | :---: | ---: |\n| 1 2 |  | 3 |",
    );
  });
});

describe("gridToText / gridToHtml", () => {
  test("tab rows and an escaped table", () => {
    expect(gridToText([["a", "b"], ["c", "d"]])).toBe("a\tb\nc\td");
    expect(gridToHtml([["<a>", "b&c"]])).toBe("<table><tbody><tr><td>&lt;a&gt;</td><td>b&amp;c</td></tr></tbody></table>");
  });
});

import { describe, expect, test } from "bun:test";
import { cellChange, cellText, movedColumns, movedRows, parseTable, removeCells } from "@/components/documents/editor/live-preview/tableModel";

const layoutFor = (source: string) => parseTable(source)!;

function edit(source: string, change: { from: number; to: number; insert: string } | null) {
  return change ? source.slice(0, change.from) + change.insert + source.slice(change.to) : source;
}

describe("table source transformations", () => {
  test("cell edits preserve other bytes and escaped pipes remain in their cell", () => {
    const source = "|  A  | B |\n| :--- | ---: |\n| x\\|y | z |";
    const layout = layoutFor(source);
    expect(layout.align).toEqual(["left", "right"]);
    expect(cellText(layout, 1, 0)).toBe("x|y");
    const changed = edit(source, cellChange(layout, 0, 1, 1, "p|q"));
    expect(changed).toBe("|  A  | B |\n| :--- | ---: |\n| x\\|y | p\\|q |");
    expect(cellText(layoutFor(changed), 1, 1)).toBe("p|q");
  });

  test("editing a short row pads omitted columns without moving existing text", () => {
    const source = "| A | B | C |\n| --- | --- | --- |\n| x |";
    const changed = edit(source, cellChange(layoutFor(source), 0, 1, 2, "z"));
    expect(changed).toBe("| A | B | C |\n| --- | --- | --- |\n| x |   | z |");
    expect(cellText(layoutFor(changed), 1, 1)).toBe("");
    expect(cellText(layoutFor(changed), 1, 2)).toBe("z");
  });

  test("a trailing odd backslash cannot escape the closing pipe", () => {
    const source = "| A | B |\n| --- | --- |\n| x | y |";
    const changed = edit(source, cellChange(layoutFor(source), 0, 1, 0, "z\\"));
    expect(layoutFor(changed).rows[1].cells).toHaveLength(2);
    expect(cellText(layoutFor(changed), 1, 1)).toBe("y");
  });

  test("moving rows keeps their bytes and the header and delimiter in place", () => {
    const source = "| A | B |\n| --- | --- |\n|  first  | 1 |\n| second | 2 |";
    expect(movedRows(layoutFor(source), 1, 3)).toBe("| A | B |\n| --- | --- |\n| second | 2 |\n|  first  | 1 |");
  });

  test("moving columns keeps alignments, missing cells, and extra cells", () => {
    const source = "| A | B |\n| :--- | ---: |\n| x |\n| y | z | extra |";
    const changed = layoutFor(movedColumns(layoutFor(source), 0, 2));
    expect(changed.align).toEqual(["right", "left"]);
    expect(cellText(changed, 1, 0)).toBe("");
    expect(cellText(changed, 1, 1)).toBe("x");
    expect(changed.rows[2].cells).toHaveLength(3);
  });

  test("removing outer cells from pipe-less rows retains a parseable boundary", () => {
    const source = "A | B | C\n--- | --- | ---\nx | y | z";
    const row = layoutFor(source).rows[1];
    const changed = edit(source, removeCells(row, row.at, 0, 0));
    expect(changed.endsWith("| y | z")).toBe(true);
  });
});

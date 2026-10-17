import { describe, expect, test } from "bun:test";
import { Text } from "@codemirror/state";

import { endsInTable, tailPadding } from "@/components/documents/editor/live-preview/livePreview/tableTail";

const TABLE = "| Path | Label |\n| --- | --- |\n| a | b |";

describe("endsInTable", () => {
  test("a table, with or without text above it", () => {
    expect(endsInTable(TABLE)).toBe(true);
    expect(endsInTable(`para\n${TABLE}`)).toBe(true);
    expect(endsInTable("a | b\n--- | ---\n1 | 2")).toBe(true);
  });

  test("not when text follows it after a blank line, or no header matches", () => {
    expect(endsInTable(`${TABLE}\n\nafter`)).toBe(false);
    expect(endsInTable("a | b\n---")).toBe(false);
    expect(endsInTable("plain\ntext")).toBe(false);
  });

  test("a fence or quote ends the table", () => {
    expect(endsInTable(`${TABLE}\n\`\`\``)).toBe(false);
    expect(endsInTable(`${TABLE}\n> quote`)).toBe(false);
  });
});

describe("tailPadding", () => {
  const doc = (s: string) => Text.of(s.split("\n"));

  test("a blank line is added before the text that follows on the same line", () => {
    expect(tailPadding(doc("rest"), 0, TABLE)).toBe("\n\n");
    expect(tailPadding(doc("rest"), 0, `${TABLE}\n`)).toBe("\n");
  });

  test("at a line's end, only when the next line has text", () => {
    expect(tailPadding(doc("\nx"), 0, TABLE)).toBe("\n");
    expect(tailPadding(doc("\n\nx"), 0, TABLE)).toBe("");
    expect(tailPadding(doc(""), 0, TABLE)).toBe("");
    expect(tailPadding(doc("\nx"), 0, `${TABLE}\n`)).toBe("");
  });

  test("none for text that does not end in a table or already ends blank", () => {
    expect(tailPadding(doc("rest"), 0, "para")).toBe("");
    expect(tailPadding(doc("rest"), 0, `${TABLE}\n\n`)).toBe("");
  });
});

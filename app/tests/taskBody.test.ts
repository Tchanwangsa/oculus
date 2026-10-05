import { describe, expect, test } from "bun:test";
import { taskBodyEdit } from "../src/lib/projects";

describe("task body edits", () => {
  test("an unchanged body writes nothing, surrounding whitespace included", () => {
    expect(taskBodyEdit("Read week 3", "Read week 3")).toBeUndefined();
    expect(taskBodyEdit("  Read week 3\n\n", "Read week 3")).toBeUndefined();
    expect(taskBodyEdit("", null)).toBeUndefined();
    expect(taskBodyEdit(" \n", null)).toBeUndefined();
  });

  test("an edit writes the trimmed text", () => {
    expect(taskBodyEdit("Read week 4\n", "Read week 3")).toBe("Read week 4");
    expect(taskBodyEdit("First note", null)).toBe("First note");
  });

  test("emptying a body clears it", () => {
    expect(taskBodyEdit("   ", "Read week 3")).toBeNull();
  });
});

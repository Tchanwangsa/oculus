import { describe, expect, test } from "bun:test";
import { columnVariant } from "@/components/projects/tasks/columnVariant";
import { DEFAULT_COLUMNS, type ProjectColumn } from "@/lib/planning/projects";

const col = (id: string, kind: ProjectColumn["kind"]): ProjectColumn => ({ id, name: id, kind });

describe("columnVariant", () => {
  test("the default board has four distinct variants", () => {
    expect(DEFAULT_COLUMNS.map((c) => columnVariant(DEFAULT_COLUMNS, c.id))).toEqual([
      "backlog",
      "todo",
      "doing",
      "done",
    ]);
  });

  test("custom active columns rank: the last is in flight, earlier ones queued", () => {
    const board = [col("a", "backlog"), col("b", "active"), col("c", "active"), col("d", "done")];
    expect(columnVariant(board, "b")).toBe("todo");
    expect(columnVariant(board, "c")).toBe("doing");
    expect(columnVariant([col("x", "active")], "x")).toBe("doing");
  });

  test("a column the board does not have has no variant", () => {
    expect(columnVariant(DEFAULT_COLUMNS, "gone")).toBeNull();
  });
});

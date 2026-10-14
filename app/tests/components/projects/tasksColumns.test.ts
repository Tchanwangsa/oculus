import { expect, test } from "bun:test";
import {
  columnIds,
  gridTemplate,
  tableMinWidth,
} from "@/components/projects/lists/tasksColumns";

test("Name is first, then the shown properties in their order", () => {
  expect(columnIds(["due", "updated"])).toEqual(["name", "due", "updated"]);
  expect(columnIds([])).toEqual(["name"]);
});

test("the grid template gives Name the remainder and every property its width", () => {
  expect(gridTemplate(columnIds(["due", "updated"]))).toBe("minmax(240px,1fr) 140px 100px");
  expect(gridTemplate(columnIds([]))).toBe("minmax(240px,1fr)");
});

test("the table scrolls sideways below the sum of its columns, gaps and gutters", () => {
  // 46 chrome + 240 + 140 + 100 + 2 gaps of 12.
  expect(tableMinWidth(columnIds(["due", "updated"]))).toBe(46 + 240 + 140 + 100 + 24);
  expect(tableMinWidth(columnIds([]))).toBe(46 + 240);
});

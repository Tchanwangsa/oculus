import { describe, expect, test } from "bun:test";
import {
  DEFAULT_DISPLAY,
  hiddenProperties,
  hideAllProperties,
  hideProperty,
  isDefaultDisplay,
  moveProperty,
  readDisplay,
  resetDisplay,
  showAllProperties,
  showProperty,
  sortByHeader,
  writeDisplay,
  type TasksDisplay,
} from "@/components/projects/display/settings";

describe("reading the stored display", () => {
  test("nothing, malformed JSON and a non-object all give the default", () => {
    expect(readDisplay(null)).toEqual(DEFAULT_DISPLAY);
    expect(readDisplay("{")).toEqual(DEFAULT_DISPLAY);
    expect(readDisplay("[1,2]")).toEqual(DEFAULT_DISPLAY);
    expect(readDisplay("7")).toEqual(DEFAULT_DISPLAY);
  });

  test("a written display reads back unchanged", () => {
    const display: TasksDisplay = {
      view: "board",
      group: "tag",
      sort: { key: "updated", dir: "desc" },
      showCompleted: false,
      properties: ["due", "status"],
    };
    expect(readDisplay(writeDisplay(display))).toEqual(display);
  });

  test("each field falls back on its own", () => {
    const read = readDisplay(
      JSON.stringify({
        view: "grid",
        group: "status",
        sort: { key: "colour", dir: "desc" },
        showCompleted: "yes",
        properties: ["tags"],
      }),
    );
    expect(read.view).toBe(DEFAULT_DISPLAY.view);
    expect(read.group).toBe("status");
    expect(read.sort).toEqual({ key: DEFAULT_DISPLAY.sort.key, dir: "desc" });
    expect(read.showCompleted).toBe(DEFAULT_DISPLAY.showCompleted);
    expect(read.properties).toEqual(["tags"]);
  });

  test("an unknown or repeated property id is dropped, and Name is not a property", () => {
    const read = readDisplay(JSON.stringify({ properties: ["due", "assignee", "due", "name", "status"] }));
    expect(read.properties).toEqual(["due", "status"]);
  });

  test("an empty list is a choice, a missing one is not", () => {
    expect(readDisplay(JSON.stringify({ properties: [] })).properties).toEqual([]);
    expect(readDisplay(JSON.stringify({ properties: "status" })).properties).toEqual(
      DEFAULT_DISPLAY.properties,
    );
    expect(readDisplay("{}").properties).toEqual(DEFAULT_DISPLAY.properties);
  });

  test("a property absent from the stored list is hidden, so a new one starts hidden", () => {
    const read = readDisplay(JSON.stringify({ properties: ["status"] }));
    expect(hiddenProperties(read)).toEqual(["tags", "project", "subject", "due", "updated", "created"]);
  });

  test("the default shows Status, Tags, Project, Due and Updated, hiding Subject and Created", () => {
    expect(DEFAULT_DISPLAY.properties).toEqual(["status", "tags", "project", "due", "updated"]);
    expect(hiddenProperties(DEFAULT_DISPLAY)).toEqual(["subject", "created"]);
  });
});

describe("editing properties", () => {
  test("hiding drops one, showing appends it last", () => {
    const hidden = hideProperty(DEFAULT_DISPLAY, "tags");
    expect(hidden.properties).toEqual(["status", "project", "due", "updated"]);
    expect(showProperty(hidden, "tags").properties).toEqual(["status", "project", "due", "updated", "tags"]);
    expect(showProperty(DEFAULT_DISPLAY, "status")).toBe(DEFAULT_DISPLAY);
  });

  test("Hide all empties the list and Show all appends the rest in order", () => {
    expect(hideAllProperties(DEFAULT_DISPLAY).properties).toEqual([]);
    expect(showAllProperties(DEFAULT_DISPLAY).properties).toEqual([
      "status",
      "tags",
      "project",
      "due",
      "updated",
      "subject",
      "created",
    ]);
  });

  test("moving stays inside the list", () => {
    expect(moveProperty(DEFAULT_DISPLAY, "tags", -1).properties[0]).toBe("tags");
    expect(moveProperty(DEFAULT_DISPLAY, "status", -1)).toBe(DEFAULT_DISPLAY);
    expect(moveProperty(DEFAULT_DISPLAY, "updated", 1)).toBe(DEFAULT_DISPLAY);
  });

  test("resetting gives the default as a copy, not the shared object", () => {
    const reset = resetDisplay();
    expect(isDefaultDisplay(reset)).toBe(true);
    reset.properties.push("created");
    expect(DEFAULT_DISPLAY.properties).not.toContain("created");
    expect(isDefaultDisplay(hideProperty(DEFAULT_DISPLAY, "due"))).toBe(false);
  });
});

describe("a column header click", () => {
  test("the same key flips direction, a new key starts ascending", () => {
    const sort = { key: "due", dir: "asc" } as const;
    expect(sortByHeader(sort, "due")).toEqual({ key: "due", dir: "desc" });
    expect(sortByHeader({ key: "due", dir: "desc" }, "due")).toEqual({ key: "due", dir: "asc" });
    expect(sortByHeader({ key: "due", dir: "desc" }, "title")).toEqual({ key: "title", dir: "asc" });
  });
});

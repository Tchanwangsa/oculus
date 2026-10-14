import { describe, expect, test } from "bun:test";
import {
  DEFAULT_DISPLAY,
  DISPLAY_KEY,
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

describe("the default display", () => {
  test("sorts by due date ascending and hides finished tasks", () => {
    expect(DEFAULT_DISPLAY.sort).toEqual({ key: "due", dir: "asc" });
    expect(DEFAULT_DISPLAY.showCompleted).toBe(false);
    expect(DEFAULT_DISPLAY.groupSubtasks).toBe(false);
    expect(readDisplay("{}").groupSubtasks).toBe(false);
    expect(readDisplay(JSON.stringify({ groupSubtasks: "yes" })).groupSubtasks).toBe(false);
    expect(readDisplay(null).showCompleted).toBe(false);
    expect(isDefaultDisplay(resetDisplay())).toBe(true);
  });

  test("is stored under a key the first Tasks page never wrote", () => {
    expect(DISPLAY_KEY).not.toBe("oculus-tasks-display");
  });
});

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
      groupSubtasks: true,
      properties: ["due", "project"],
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
    const read = readDisplay(JSON.stringify({ properties: ["due", "assignee", "due", "name", "project"] }));
    expect(read.properties).toEqual(["due", "project"]);
  });

  test("an empty list is a choice, a missing one is not", () => {
    expect(readDisplay(JSON.stringify({ properties: [] })).properties).toEqual([]);
    expect(readDisplay(JSON.stringify({ properties: "tags" })).properties).toEqual(
      DEFAULT_DISPLAY.properties,
    );
    expect(readDisplay("{}").properties).toEqual(DEFAULT_DISPLAY.properties);
  });

  test("a property absent from the stored list is hidden, so a new one starts hidden", () => {
    const read = readDisplay(JSON.stringify({ properties: ["tags"] }));
    expect(hiddenProperties(read)).toEqual(["project", "subject", "due", "updated", "created"]);
  });

  test("the default shows Tags, Project, Due and Updated, hiding Subject and Created", () => {
    expect(DEFAULT_DISPLAY.properties).toEqual(["tags", "project", "due", "updated"]);
    expect(hiddenProperties(DEFAULT_DISPLAY)).toEqual(["subject", "created"]);
  });
});

describe("editing properties", () => {
  test("hiding drops one, showing appends it last", () => {
    const hidden = hideProperty(DEFAULT_DISPLAY, "tags");
    expect(hidden.properties).toEqual(["project", "due", "updated"]);
    expect(showProperty(hidden, "tags").properties).toEqual(["project", "due", "updated", "tags"]);
    expect(showProperty(DEFAULT_DISPLAY, "tags")).toBe(DEFAULT_DISPLAY);
  });

  test("Hide all empties the list and Show all appends the rest in order", () => {
    expect(hideAllProperties(DEFAULT_DISPLAY).properties).toEqual([]);
    expect(showAllProperties(DEFAULT_DISPLAY).properties).toEqual([
      "tags",
      "project",
      "due",
      "updated",
      "subject",
      "created",
    ]);
  });

  test("moving stays inside the list", () => {
    expect(moveProperty(DEFAULT_DISPLAY, "project", -1).properties[0]).toBe("project");
    expect(moveProperty(DEFAULT_DISPLAY, "tags", -1)).toBe(DEFAULT_DISPLAY);
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

test("saved Status columns are dropped while other display choices survive", () => {
  const read = readDisplay(JSON.stringify({ properties: ["status", "due", "tags"], groupSubtasks: true, group: "project" }));
  expect(read.properties).toEqual(["due", "tags"]);
  expect(read.groupSubtasks).toBe(true);
  expect(read.group).toBe("project");
});

describe("a column header click", () => {
  test("the same key flips direction, a new key starts ascending", () => {
    const sort = { key: "due", dir: "asc" } as const;
    expect(sortByHeader(sort, "due")).toEqual({ key: "due", dir: "desc" });
    expect(sortByHeader({ key: "due", dir: "desc" }, "due")).toEqual({ key: "due", dir: "asc" });
    expect(sortByHeader({ key: "due", dir: "desc" }, "title")).toEqual({ key: "title", dir: "asc" });
  });
});

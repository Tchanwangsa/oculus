import { describe, expect, test } from "bun:test";
import { clickSelect, EMPTY_SELECTION, pruneSelection, selectAll, selectedInOrder } from "@/lib/drive/selection";

const ORDER = ["a", "b", "c", "d", "e"];
const keys = (s: { keys: ReadonlySet<string> }) => [...s.keys].sort();
const plain = { toggle: false, range: false };

describe("clickSelect", () => {
  test("a plain click picks one and anchors there", () => {
    const s = clickSelect({ keys: new Set(["a", "b"]), anchor: "a" }, "c", ORDER, plain);
    expect(keys(s)).toEqual(["c"]);
    expect(s.anchor).toBe("c");
  });

  test("⌘-click toggles one and moves the anchor", () => {
    let s = clickSelect(EMPTY_SELECTION, "b", ORDER, plain);
    s = clickSelect(s, "d", ORDER, { toggle: true, range: false });
    expect(keys(s)).toEqual(["b", "d"]);
    expect(s.anchor).toBe("d");
    s = clickSelect(s, "b", ORDER, { toggle: true, range: false });
    expect(keys(s)).toEqual(["d"]);
  });

  test("⇧-click takes the run from the anchor, either way, and keeps the anchor", () => {
    let s = clickSelect(EMPTY_SELECTION, "b", ORDER, plain);
    s = clickSelect(s, "d", ORDER, { toggle: false, range: true });
    expect(keys(s)).toEqual(["b", "c", "d"]);
    s = clickSelect(s, "a", ORDER, { toggle: false, range: true });
    expect(keys(s)).toEqual(["a", "b"]);
    expect(s.anchor).toBe("b");
  });

  test("⌘⇧-click adds the run to what is picked", () => {
    let s = clickSelect(EMPTY_SELECTION, "a", ORDER, plain);
    s = clickSelect(s, "c", ORDER, { toggle: true, range: false });
    s = clickSelect(s, "e", ORDER, { toggle: true, range: true });
    expect(keys(s)).toEqual(["a", "c", "d", "e"]);
  });

  test("⇧-click with no anchor picks just the one", () => {
    const s = clickSelect(EMPTY_SELECTION, "c", ORDER, { toggle: false, range: true });
    expect(keys(s)).toEqual(["c"]);
  });
});

test("⌘A picks everything on screen", () => {
  expect(keys(selectAll(ORDER))).toEqual(ORDER);
  expect(selectAll([]).anchor).toBeNull();
});

test("pruning drops entries that left and keeps identity otherwise", () => {
  const s = { keys: new Set(["a", "x"]), anchor: "x" };
  const pruned = pruneSelection(s, ORDER);
  expect(keys(pruned)).toEqual(["a"]);
  expect(pruned.anchor).toBeNull();
  const kept = { keys: new Set(["a"]), anchor: "a" };
  expect(pruneSelection(kept, ORDER)).toBe(kept);
});

test("selected keys come back in screen order", () => {
  expect(selectedInOrder({ keys: new Set(["d", "a"]), anchor: null }, ORDER)).toEqual(["a", "d"]);
});

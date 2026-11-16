import { afterEach, describe, expect, test } from "bun:test";
import { File } from "@phosphor-icons/react";
import { MIN_COLUMN, RAIL, RESERVE, threadInfoFloats } from "@/components/harness/threadInfo/layout";
import { registerWorkKind, workKinds, type WorkKind } from "@/components/harness/threadInfo/work";

describe("threadInfoFloats: the card floats only beside a column of at least MIN_COLUMN", () => {
  // The narrowest pane that fits: the rail's gutter, the narrowest column
  // and the card's reserve.
  const least = RAIL + MIN_COLUMN + RESERVE;

  test("a wide pane floats the card; one pixel narrower makes it a popover", () => {
    expect(threadInfoFloats(least, 800, 300)).toBe(true);
    expect(threadInfoFloats(least - 1, 800, 300)).toBe(false);
  });

  test("a side panel open at half width leaves no room", () => {
    expect(threadInfoFloats(1440 / 2, 800, 300)).toBe(false);
  });

  test("a card taller than the pane is a popover, since it never scrolls", () => {
    expect(threadInfoFloats(1400, 400, 376)).toBe(true);
    expect(threadInfoFloats(1400, 400, 377)).toBe(false);
  });

  test("before the card has drawn, the width alone decides", () => {
    expect(threadInfoFloats(1400, 200, 0)).toBe(true);
  });
});

describe("Work kinds", () => {
  const removals: (() => void)[] = [];
  afterEach(() => removals.splice(0).forEach((r) => r()));
  const kind = (id: string, title = id): WorkKind => ({
    id,
    useRows: () => [{ key: id, icon: File, title, href: `/x/${id}` }],
  });

  test("none are registered until a feature adds one", () => {
    expect(workKinds()).toEqual([]);
  });

  test("kinds keep their first registration's order, and a re-registration replaces in place", () => {
    removals.push(registerWorkKind(kind("video")), registerWorkKind(kind("quiz")));
    const again = kind("video", "Video v2");
    removals.push(registerWorkKind(again));
    expect(workKinds().map((k) => k.id)).toEqual(["video", "quiz"]);
    expect(workKinds()[0]).toBe(again);
  });

  test("a removal takes only its own kind, once", () => {
    const remove = registerWorkKind(kind("video"));
    removals.push(registerWorkKind(kind("quiz")));
    remove();
    remove();
    expect(workKinds().map((k) => k.id)).toEqual(["quiz"]);
  });
});

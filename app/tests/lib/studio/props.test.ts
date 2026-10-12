import { describe, expect, test } from "bun:test";

import type { NamedProp } from "@/lib/studio/components";
import { changed, describeProp, groupProps } from "@/lib/studio/props";

const num = (name: string, def?: number): NamedProp => ({ name, kind: "num", optional: false, ...(def === undefined ? {} : { default: def }) });
const text = (name: string): NamedProp => ({ name, kind: "str", optional: true });
const tone = (name: string): NamedProp => ({ name, kind: "token", token: "color", optional: false, default: "accent" });
const list = (name: string, of: NamedProp, def?: unknown[]): NamedProp => ({
  name,
  kind: "list",
  of,
  optional: false,
  ...(def === undefined ? {} : { default: def }),
});

describe("groupProps", () => {
  const axes: NamedProp[] = [
    list("x", num("n"), [-1, 4, 1]),
    text("xTitle"),
    list("tones", tone("t"), []),
    num("at", 0.2),
    num("plotAt", 1.4),
    { name: "grid", kind: "bool", optional: false, default: true },
    { name: "enter", kind: "oneOf", optional: false, values: ["rise", "fade"], default: "rise" },
    num("zoomDur", 1.4),
  ];

  test("sorts a schema into content, style and timing, keeping declaration order", () => {
    const groups = groupProps(axes);
    expect(groups.map((g) => [g.title, g.props.map((p) => p.name)])).toEqual([
      ["Content", ["x", "xTitle"]],
      ["Style", ["tones", "grid", "enter"]],
      ["Timing", ["at", "plotAt", "zoomDur"]],
    ]);
  });

  test("a name that ends in a moment is timing, whatever its kind", () => {
    const groups = groupProps([...axes, list("markAt", num("n"), []), list("revealEach", num("n"))]);
    const timing = groups.find((g) => g.id === "timing")!;
    expect(timing.props.map((p) => p.name)).toEqual(["at", "plotAt", "zoomDur", "markAt", "revealEach"]);
  });

  test("few props, or all in one group, stay one Props section", () => {
    expect(groupProps(axes.slice(0, 3)).map((g) => g.id)).toEqual(["props"]);
    const flat = ["a", "b", "c", "d", "e", "f", "g"].map((n) => text(n));
    const groups = groupProps(flat);
    expect(groups).toHaveLength(1);
    expect(groups[0]).toMatchObject({ id: "props", title: "Props" });
    expect(groups[0].props).toHaveLength(7);
    expect(groupProps([])).toEqual([{ id: "props", title: "Props", props: [] }]);
  });
});

describe("describeProp", () => {
  test("says what a prop takes and what it defaults to", () => {
    expect(describeProp(list("x", num("n"), [-1, 4, 1]))).toBe("List of numbers · default [-1,4,1]");
    expect(describeProp(text("xTitle"))).toBe("Text · optional");
    expect(describeProp(num("at"))).toBe("Number · required");
    expect(describeProp(tone("tone"))).toBe('A name in color · default "accent"');
    expect(describeProp({ name: "e", kind: "oneOf", optional: false, values: ["a", 2] })).toBe("One of a, 2 · required");
  });
});

describe("changed", () => {
  test("is set to something other than the default, and never for a required prop", () => {
    const at = num("at", 0.2);
    expect(changed(at, {})).toBe(false);
    expect(changed(at, { at: 0.2 })).toBe(false);
    expect(changed(at, { at: 0.5 })).toBe(true);
    expect(changed(list("x", num("n"), [1, 2]), { x: [1, 2] })).toBe(false);
    expect(changed(list("x", num("n"), [1, 2]), { x: [1, 3] })).toBe(true);
    expect(changed(text("xTitle"), { xTitle: "a" })).toBe(true);
    expect(changed(num("required"), { required: 3 })).toBe(false);
  });
});

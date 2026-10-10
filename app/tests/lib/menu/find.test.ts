import { describe, expect, test } from "bun:test";
import { buildCorpus, findMatches, findPattern, locate } from "@/lib/menu/findText";
import { pickFindTarget, type FindScene } from "@/lib/menu/find";

function corpus(...parts: (string | null)[]) {
  // `null` marks a block boundary before the next part.
  const segments = [];
  let breakBefore = false;
  for (const p of parts) {
    if (p === null) breakBefore = true;
    else {
      segments.push({ text: p, breakBefore });
      breakBefore = false;
    }
  }
  return buildCorpus(segments);
}

function spans(parts: (string | null)[], query: string) {
  const c = corpus(...parts);
  return findMatches(c, query, 100).matches.map((m) => [
    locate(c, m.start, false),
    locate(c, m.end, true),
  ]);
}

describe("DOM find text", () => {
  test("matches case-insensitively across adjacent inline text nodes", () => {
    expect(spans(["Hel", "lo wor", "ld"], "LLO WORLD")).toEqual([
      [{ segment: 0, offset: 2 }, { segment: 2, offset: 2 }],
    ]);
  });

  test("never joins text across a block boundary", () => {
    expect(spans(["foo", null, "bar"], "foobar")).toEqual([]);
    expect(spans(["foo", null, "bar"], "foo bar")).toEqual([]);
    expect(spans(["foo ", "bar"], "foo bar")).toHaveLength(1);
  });

  test("a match ending at a node's end stays in that node", () => {
    expect(spans(["ab", "cd"], "ab")).toEqual([
      [{ segment: 0, offset: 0 }, { segment: 0, offset: 2 }],
    ]);
    expect(spans(["ab", "cd"], "cd")).toEqual([
      [{ segment: 1, offset: 0 }, { segment: 1, offset: 2 }],
    ]);
  });

  test("whitespace runs match any whitespace and regex characters are literal", () => {
    expect(spans(["a\n   b"], "a b")).toHaveLength(1);
    expect(spans(["f(x) = [1+2]*3?"], "(x) = [1+2]*3?")).toHaveLength(1);
    expect(findPattern("   ")).toBeNull();
  });

  test("caps the match count", () => {
    const c = corpus("aaaaa");
    expect(findMatches(c, "a", 3)).toEqual({
      matches: [
        { start: 0, end: 1 },
        { start: 1, end: 2 },
        { start: 2, end: 3 },
      ],
      capped: true,
    });
  });
});

/** Nodes are path strings: "a/b" is inside "a". */
type T = { name: string; root: string; page?: number; dom?: boolean };
const contains = (outer: string, inner: string) =>
  inner === outer || inner.startsWith(outer + "/");

function scene(over: Partial<FindScene<string>>): FindScene<string> {
  return {
    contains,
    hovered: () => false,
    documentFocused: true,
    focus: null,
    focusInOverlay: false,
    engaged: null,
    pane: 1,
    ...over,
  };
}

const pane1: T = { name: "pane1", root: "p1", page: 1, dom: true };
const pane2: T = { name: "pane2", root: "p2", page: 2, dom: true };
const pdf: T = { name: "pdf", root: "p1/pdf" };
const browser: T = { name: "browser", root: "p2/browser", page: 2 };
const all = [pane1, pane2, pdf, browser];
const pick = (s: Partial<FindScene<string>>) => pickFindTarget(all, scene(s))?.name;

describe("find routing", () => {
  test("focus wins, innermost first", () => {
    expect(pick({ focus: "p1/pdf/bar", hovered: (r) => r === "p2" })).toBe("pdf");
    expect(pick({ focus: "p1/chat" })).toBe("pane1");
  });

  test("focus in an overlay outside every target answers nothing", () => {
    expect(pick({ focus: "palette", focusInOverlay: true, engaged: "p1/pdf" })).toBeUndefined();
    expect(pick({ focus: "p1/menu", focusInOverlay: true })).toBe("pane1");
  });

  test("then hover, then the last engaged target, then the pane's page target", () => {
    const hovered = (r: string) => contains(r, "p1/pdf/page");
    expect(pick({ hovered, engaged: "p2" })).toBe("pdf");
    expect(pick({ engaged: "p2/browser/address" })).toBe("browser");
    expect(pick({ pane: 2 })).toBe("browser");
  });

  test("the page fallback goes to the one target inside it", () => {
    expect(pick({ pane: 1 })).toBe("pdf");
    const two = [...all, { name: "pdf2", root: "p1/pdf2" }];
    expect(pickFindTarget(two, scene({ pane: 1 }))?.name).toBe("pane1");
    expect(pickFindTarget([pane1], scene({ pane: 1 }))?.name).toBe("pane1");
  });

  test("hover and engagement on a page hand off to its one child, focus does not", () => {
    expect(pick({ hovered: (r) => contains(r, "p1/header") })).toBe("pdf");
    expect(pick({ engaged: "p1/header" })).toBe("pdf");
    expect(pick({ focus: "p1/header/input" })).toBe("pane1");
  });

  test("the side panel's front item is a pane with its own page find", () => {
    const side: T = { name: "side", root: "s3", page: 3, dom: true };
    const sidePdf: T = { name: "sidePdf", root: "s3/pdf" };
    const both = [pane1, pdf, side, sidePdf];
    expect(pickFindTarget(both, scene({ pane: 3 }))?.name).toBe("sidePdf");
    expect(pickFindTarget(both, scene({ pane: 1 }))?.name).toBe("pdf");
    expect(pickFindTarget(both, scene({ pane: 1, hovered: (r) => contains(r, "s3/top") }))?.name).toBe("sidePdf");
    expect(pickFindTarget(both, scene({ pane: 3, focus: "s3/title" }))?.name).toBe("side");
  });

  test("a DOM find is not a child to hand off to, nor what it holds", () => {
    const inner: T = { name: "inner", root: "p1/inner", dom: true };
    const innerPdf: T = { name: "innerPdf", root: "p1/inner/pdf" };
    const nested = [pane1, inner, innerPdf];
    expect(pickFindTarget(nested, scene({ pane: 1 }))?.name).toBe("pane1");
    const overHeader = scene({ hovered: (r) => contains(r, "p1/header") });
    expect(pickFindTarget(nested, overHeader)?.name).toBe("pane1");
    expect(pickFindTarget([...nested, pdf], overHeader)?.name).toBe("pdf");
    const overInner = scene({ hovered: (r) => contains(r, "p1/inner/header") });
    expect(pickFindTarget(nested, overInner)?.name).toBe("innerPdf");
  });

  test("a page with two editors keeps ⌘F", () => {
    const two = [pane1, { name: "a", root: "p1/a" }, { name: "b", root: "p1/b" }];
    expect(pickFindTarget(two, scene({ hovered: (r) => contains(r, "p1/title") }))?.name).toBe("pane1");
    expect(pickFindTarget(two, scene({ pane: 1 }))?.name).toBe("pane1");
  });

  test("a native browser page holding the keyboard skips focus, hover and engagement", () => {
    expect(
      pick({ documentFocused: false, pane: 2, focus: "p1/pdf", hovered: () => true, engaged: "p1" }),
    ).toBe("browser");
  });
});

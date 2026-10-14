import { describe, expect, test } from "bun:test";
import {
  anchorDrift,
  firstBelow,
  inSkippedSubtree,
  pickAnchor,
  SKIPPED_CONTENT,
  zoomScale,
  type AnchorProbe,
} from "@/lib/ui/scrollAnchor";

type Box = {
  name: string;
  top: number;
  height: number;
  display?: string;
  skipped?: boolean;
  placeholder?: boolean;
  children?: Box[];
};

/** Fake elements: rects in viewport px, as page zoom would scale them. */
function build(box: Box, moved = { by: 0 }): Element {
  const children = (box.children ?? []).map((c) => build(c, moved));
  const el = {
    name: box.name,
    box,
    isConnected: true,
    children,
    firstElementChild: children[0] ?? null,
    hasAttribute: (name: string) => name === SKIPPED_CONTENT && !!box.placeholder,
    getBoundingClientRect: () => {
      const top = box.top + moved.by;
      return { top, bottom: top + box.height, height: box.height };
    },
    contains: (other: Element) => other === el || children.some((c) => c.contains(other)),
  };
  return el as unknown as Element;
}

const probe: AnchorProbe = {
  display: (el) => (el as unknown as { box: Box }).box.display ?? "block",
  skipped: (el) => (el as unknown as { box: Box }).box.skipped ?? false,
};

/** A scroller whose top sits at viewport y 100, `zoom` rect px per layout px. */
function scroller(zoom = 1, height = 600) {
  return {
    offsetHeight: height,
    getBoundingClientRect: () => ({ top: 100, bottom: 100 + height * zoom, height: height * zoom }),
  } as unknown as HTMLElement;
}

const name = (el: Element | undefined) => (el as unknown as { name: string } | undefined)?.name;

describe("scroll anchor", () => {
  test("explicitly skipped contents are ignored even when the visibility API returns false both ways", () => {
    const row = {} as Element;
    const child = { closest: () => row, checkVisibility: () => false } as unknown as Element;
    expect(inSkippedSubtree(child)).toBe(true);
  });

  test("an explicitly skipped row still has an anchorable box", () => {
    const row = { closest: () => row, checkVisibility: () => false } as unknown as Element;
    expect(inSkippedSubtree(row)).toBe(false);
  });

  test("automatic skipping differs from an element without a box", () => {
    const auto = {
      closest: () => null,
      checkVisibility: (options?: { contentVisibilityAuto?: boolean }) => !options?.contentVisibilityAuto,
    } as unknown as Element;
    const boxless = { closest: () => null, checkVisibility: () => false } as unknown as Element;
    expect(inSkippedSubtree(auto)).toBe(true);
    expect(inSkippedSubtree(boxless)).toBe(false);
  });

  test("a placeholder anchor can be refined to its paragraph after reconnecting", () => {
    const stub = build({ name: "row", top: -100, height: 500, placeholder: true });
    const deferred = pickAnchor(scroller(), [stub], probe)!;
    expect(deferred.node).toBe(stub);
    expect(deferred.deferred).toBe(true);
    const drawn = build({ name: "row", top: -100, height: 500, children: [
      { name: "body", top: -100, height: 500, children: [{ name: "paragraph", top: 80, height: 180 }] },
    ] });
    const refined = pickAnchor(scroller(), [drawn], probe)!;
    expect(name(refined.node)).toBe("paragraph");
    expect(refined.deferred).toBe(false);
    expect(refined.offset).toBe(-20);
  });

  test("zoomScale is rect px per layout px, and 1 for an unlaid box", () => {
    expect(zoomScale(690, 600)).toBeCloseTo(1.15);
    expect(zoomScale(0, 600)).toBe(1);
    expect(zoomScale(600, 0)).toBe(1);
  });

  test("firstBelow finds the first bottom past the edge", () => {
    const bottoms = [10, 40, 90, 200, 300];
    const at = (i: number) => bottoms[i];
    expect(firstBelow(bottoms.length, at, 0)).toBe(0);
    expect(firstBelow(bottoms.length, at, 40)).toBe(2);
    expect(firstBelow(bottoms.length, at, 41)).toBe(2);
    expect(firstBelow(bottoms.length, at, 299)).toBe(4);
    expect(firstBelow(bottoms.length, at, 300)).toBe(-1);
    expect(firstBelow(0, at, 0)).toBe(-1);
  });

  test("anchors the row crossing the top, down to the block being read", () => {
    const rows = [
      build({ name: "q", top: -300, height: 100 }),
      build({
        name: "reply",
        top: -190,
        height: 900,
        children: [
          {
            name: "md",
            top: -190,
            height: 860,
            children: [
              { name: "p1", top: -190, height: 200 },
              { name: "p2", top: -10, height: 200, children: [{ name: "strong", top: 90, height: 20, display: "inline" }] },
            ],
          },
        ],
      }),
    ];
    const anchor = pickAnchor(scroller(), rows, probe);
    // p2 crosses the edge at 100; its inline child is not followed.
    expect(name(anchor?.node)).toBe("p2");
    expect(anchor?.offset).toBe(-110);
  });

  test("offsets are in layout px at a page zoom", () => {
    const rows = [build({ name: "row", top: 330, height: 50 })];
    const anchor = pickAnchor(scroller(1.15), rows, probe);
    expect(anchor?.offset).toBeCloseTo(200);
  });

  test("a skipped row crossing the edge hands the anchor to the next row", () => {
    const rows = [
      build({ name: "skipped", top: -100, height: 260, children: [{ name: "c", top: -100, height: 260, skipped: true }] }),
      build({ name: "next", top: 168, height: 40 }),
    ];
    expect(name(pickAnchor(scroller(), rows, probe)?.node)).toBe("next");
  });

  test("no rows, or all above the top, give no anchor", () => {
    expect(pickAnchor(scroller(), [], probe)).toBeNull();
    expect(pickAnchor(scroller(), [build({ name: "a", top: -50, height: 100 })], probe)).toBeNull();
  });

  test("drift is how far the anchor moved, in scroll px", () => {
    const moved = { by: 0 };
    const row = build({ name: "row", top: 300, height: 50 }, moved);
    const content = build({ name: "content", top: 0, height: 1000 });
    (content as unknown as { contains: (e: Element) => boolean }).contains = (e) => e === row;
    const view = scroller(1.15);
    const anchor = pickAnchor(view, [row], probe)!;
    expect(anchorDrift(view, anchor, content, probe)).toBeCloseTo(0);
    moved.by = 115;
    expect(anchorDrift(view, anchor, content, probe)).toBeCloseTo(100);
    moved.by = -23;
    expect(anchorDrift(view, anchor, content, probe)).toBeCloseTo(-20);
  });

  test("a removed anchor reports null so the caller re-picks", () => {
    const row = build({ name: "row", top: 300, height: 50 });
    const content = build({ name: "content", top: 0, height: 1000 });
    const view = scroller();
    const anchor = pickAnchor(view, [row], probe)!;
    expect(anchorDrift(view, anchor, content, probe)).toBeNull();
  });
});

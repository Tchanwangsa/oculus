import { describe, expect, test } from "bun:test";
import { nearestScroll } from "@/components/documents/editor/math/field/noteScroll";

describe("nearestScroll", () => {
  // The visible band: under a 44px toolbar, down to 600.
  const [top, bottom] = [49, 595];

  test("a caret already in view moves nothing", () => {
    expect(nearestScroll(100, 117, top, bottom)).toBe(0);
    expect(nearestScroll(49, 66, top, bottom)).toBe(0);
    expect(nearestScroll(578, 595, top, bottom)).toBe(0);
  });

  test("a caret below the band scrolls down just to its bottom edge", () => {
    expect(nearestScroll(590, 607, top, bottom)).toBe(12);
    expect(nearestScroll(900, 917, top, bottom)).toBe(322);
  });

  test("a caret above it, or under the toolbar, scrolls up to its top edge", () => {
    expect(nearestScroll(22, 39, top, bottom)).toBe(-27);
    expect(nearestScroll(-300, -283, top, bottom)).toBe(-349);
  });

  test("a caret taller than the band keeps its top in view", () => {
    expect(nearestScroll(100, 900, top, bottom)).toBe(51);
  });
});

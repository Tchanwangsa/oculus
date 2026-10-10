import { describe, expect, test } from "bun:test";
import { MathField } from "@/lib/maths";
import { bands, measure, stopAt, type Box, type Layout, type MappedBox } from "@/lib/maths/geometry";

const box = (left: number, top: number, right: number, bottom: number): Box => ({ left, top, right, bottom });

/** A mapped element; its ink is its box unless given. */
const item = (from: number, to: number, b: Box, ink: Box = b, placeholder = false): MappedBox => ({
  from,
  to,
  box: b,
  ink,
  placeholder,
});

/** `\frac{a+x}{b}` laid out by hand: the numerator's glyphs on top, the
 *  denominator under them, the fraction's own inline box on the baseline. */
const FRAC = "\\frac{a+x}{b}";
const fracLayout: Layout = {
  items: [
    item(0, 13, box(0, 10, 30, 22), box(0, 0, 30, 30)),
    item(10, 13, box(12, 18, 18, 30)),
    item(11, 12, box(12, 18, 18, 30)),
    item(5, 10, box(5, 0, 25, 12)),
    item(6, 7, box(5, 0, 11, 12)),
    item(7, 8, box(13, 0, 19, 12)),
    item(8, 9, box(20, 0, 25, 12)),
  ],
};

const stopOf = (field: MathField, offset: number, slot: number) =>
  [...field.stops].findIndex((o, id) => o === offset && field.stopSlots[id] === slot);

describe("measure", () => {
  const field = MathField.open(FRAC, false);
  const m = measure(fracLayout, field);

  test("a stop sits after the atom ending at it, sized by that atom's line", () => {
    const afterX = stopOf(field, 9, 1);
    expect([m.x[afterX], m.top[afterX], m.bottom[afterX]]).toEqual([25, 0, 12]);
  });

  test("a slot's first stop sits before its first atom", () => {
    expect(m.x[stopOf(field, 6, 1)]).toBe(5);
    expect(m.x[stopOf(field, 0, 0)]).toBe(0);
  });

  test("the widest atom wins, never an element of a slot inside it", () => {
    // The denominator's wrapper also ends at 13, inside the fraction.
    const end = stopOf(field, 13, 0);
    expect([m.x[end], m.top[end], m.bottom[end]]).toEqual([30, 10, 22]);
  });

  test("source whitespace between a stop and its atom is skipped", () => {
    const source = "\\left( x \\right)";
    const f = MathField.open(source, false);
    const layout: Layout = { items: [item(0, 16, box(0, 0, 20, 14)), item(7, 8, box(6, 2, 12, 14))] };
    const mm = measure(layout, f);
    expect(mm.x[stopOf(f, 6, 1)]).toBe(6);
    expect(mm.x[stopOf(f, 8, 1)]).toBe(12);
  });

  test("an empty slot's caret is at its placeholder", () => {
    const f = MathField.open("\\frac{}{b}", false);
    const layout: Layout = {
      items: [
        item(0, 10, box(0, 10, 20, 22), box(0, 0, 20, 30)),
        item(6, 6, box(4, 0, 12, 12), box(4, 0, 12, 12), true),
        item(8, 9, box(6, 18, 12, 30)),
      ],
    };
    const mm = measure(layout, f);
    const inNumer = stopOf(f, 6, 1);
    expect([mm.x[inNumer], mm.top[inNumer], mm.bottom[inNumer]]).toEqual([4, 0, 12]);
  });

  test("nothing mapped is NaN", () => {
    const mm = measure({ items: [] }, MathField.open("", false));
    expect(Number.isNaN(mm.x[0])).toBe(true);
  });
});

describe("stopAt", () => {
  const field = MathField.open(FRAC, false);
  const m = measure(fracLayout, field);

  test("a press in the numerator takes its nearest stop", () => {
    expect(stopAt(fracLayout, field, m, 19, 6)).toBe(stopOf(field, 8, 1));
  });

  test("a press beside the maths takes the row's nearest end", () => {
    expect(stopAt(fracLayout, field, m, 80, 15)).toBe(stopOf(field, 13, 0));
    expect(stopAt(fracLayout, field, m, -40, 15)).toBe(stopOf(field, 0, 0));
  });

  test("a display's rows: the band nearest in y", () => {
    const source = "a \\\\ b \\\\ c";
    const f = MathField.open(source, true);
    const layout: Layout = {
      items: [item(0, 1, box(10, 0, 16, 12)), item(5, 6, box(10, 20, 16, 32)), item(10, 11, box(10, 40, 16, 52))],
    };
    const mm = measure(layout, f);
    expect(stopAt(layout, f, mm, 100, 49)).toBe(stopOf(f, 11, 2));
    expect(stopAt(layout, f, mm, 0, 17)).toBe(stopOf(f, 5, 1));
  });
});

describe("bands", () => {
  test("a selection in a row is one band over its glyphs", () => {
    expect(bands(fracLayout, 6, 9)).toEqual([box(5, 0, 25, 12)]);
  });

  test("a whole structure is one band over its ink", () => {
    expect(bands(fracLayout, 0, 13)).toEqual([box(0, 0, 30, 30)]);
  });

  test("display rows each get their own band", () => {
    const layout: Layout = {
      items: [item(0, 1, box(10, 0, 16, 12)), item(5, 6, box(8, 20, 18, 32)), item(10, 11, box(12, 40, 14, 52))],
    };
    expect(bands(layout, 0, 11)).toEqual([box(10, 0, 16, 12), box(8, 20, 18, 32), box(12, 40, 14, 52)]);
  });

  test("rows drawn overlapping stay apart, cut where they meet", () => {
    // `\frac12 \\ b`: the fraction's denominator reaches into b's line.
    const layout: Layout = { items: [item(0, 8, box(0, 0, 10, 20)), item(11, 12, box(0, 16, 30, 32))] };
    const rows = [
      { from: 0, to: 8 },
      { from: 11, to: 12 },
    ];
    expect(bands(layout, 0, 12, rows)).toEqual([box(0, 0, 10, 18), box(0, 18, 30, 32)]);
    expect(bands(layout, 0, 12)).toEqual([box(0, 0, 30, 32)]);
  });

  test("an empty range has none", () => {
    expect(bands(fracLayout, 4, 4)).toEqual([]);
  });
});

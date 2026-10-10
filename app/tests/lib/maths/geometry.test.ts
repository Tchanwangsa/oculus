import { describe, expect, test } from "bun:test";
import { MathField } from "@/lib/maths";
import { bands, measure, stopAt, type Box, type Layout, type MappedBox } from "@/lib/maths/geometry";

const box = (left: number, top: number, right: number, bottom: number): Box => ({ left, top, right, bottom });

/** A mapped element; its ink is its box unless given. Its font size is its
 *  box's height and its baseline three quarters down, so a caret beside it
 *  (0.75em above the baseline, 0.25em below) spans its box exactly. */
const item = (
  from: number,
  to: number,
  b: Box,
  ink: Box = b,
  placeholder = false,
  size = b.bottom - b.top,
  baseline = b.top + 0.75 * size,
): MappedBox => ({ from, to, box: b, ink, placeholder, baseline, size });

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

  test("a stop sits after the atom ending at it, sized by that atom's font", () => {
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

  test("a caret is one em of its font on its baseline, never its box's height", () => {
    // `b^{}`: the script's `□` (KaTeX_AMS) has a box far taller than its
    // 12px font; the caret fits the script, not the base's line.
    const f = MathField.open("b^{}", false);
    const layout: Layout = {
      items: [
        item(0, 4, box(0, 1, 17, 21), box(0, -2, 17, 21), false, 16, 16),
        item(0, 1, box(0, 4, 7, 20), box(0, 4, 7, 20), false, 16, 16),
        item(3, 3, box(8, -2, 17, 15), box(8, -2, 17, 15), true, 12, 8),
      ],
    };
    const mm = measure(layout, f);
    const inSup = stopOf(f, 3, 1);
    expect([mm.x[inSup], mm.top[inSup], mm.bottom[inSup]]).toEqual([8, -1, 11]);
    const end = stopOf(f, 4, 0);
    expect([mm.top[end], mm.bottom[end]]).toEqual([4, 20]);
  });

  test("an empty slot with no marker takes the enclosing element's font, not its ink", () => {
    const f = MathField.open("\\frac{}{b}", false);
    const layout: Layout = {
      items: [item(0, 10, box(0, 10, 20, 22), box(0, 0, 20, 30)), item(8, 9, box(6, 18, 12, 30))],
    };
    const mm = measure(layout, f);
    const inNumer = stopOf(f, 6, 1);
    expect([mm.x[inNumer], mm.top[inNumer], mm.bottom[inNumer]]).toEqual([10, 10, 22]);
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

  describe("an array whose rows are all selected", () => {
    // `\frac12(…) = \begin{cases}…\end{cases}` as WebKit lays it out:
    // KaTeX draws the array column by column, inside an element for the
    // `cases` (its brace) and one for the array (its padding).
    const tex = "\\frac{1}{\\sqrt2}(|0\\rangle+(-1)^{s_i}|1\\rangle) = \\begin{cases}|+\\rangle & s_i = 0 \\text{ (no CNOT)}\\\\ |-\\rangle & s_i=1\\end{cases}";
    const slots = MathField.open(tex, true).slots;
    const layout: Layout = {
      items: [
        item(0, 16, box(116.66, 2.91, 143.32, 48.23)),
        item(16, 17, box(143.32, 14.38, 149.91, 34.38)),
        item(46, 47, box(255.18, 14, 261.77, 34)),
        item(48, 49, box(266.47, 14, 279.65, 34)),
        item(50, 131, box(284.38, -0.36, 475.34, 50.64)),
        item(50, 131, box(298.02, 1, 475.3, 49.78)),
        item(63, 73, box(298.02, 3.08, 322.5, 23.08)),
        item(103, 113, box(298.02, 27.47, 322.5, 47.47)),
        item(75, 100, box(339.44, 3.08, 473.3, 24.63)),
        item(115, 120, box(339.44, 27.47, 383.42, 49.02)),
      ],
    };

    test("gives a band per row, the atoms before it and its brace one beside them", () => {
      expect(bands(layout, 0, tex.length, slots)).toEqual([
        box(116.66, -0.36, 298.02, 50.64),
        box(298.02, 3.08, 473.3, 24.63),
        box(298.02, 27.47, 383.42, 49.02),
      ]);
    });

    test("never one box over the whole formula", () => {
      const whole = bands(layout, 0, tex.length, slots);
      expect(whole.some((b) => b.left <= 116.66 && b.right >= 473.3)).toBe(false);
    });

    test("part of a cell is one band over just those atoms", () => {
      expect(bands(layout, 75, 100, slots)).toEqual([box(339.44, 3.08, 473.3, 24.63)]);
    });
  });
});

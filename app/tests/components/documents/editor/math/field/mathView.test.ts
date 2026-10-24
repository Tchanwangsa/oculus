import { keyDown } from "@/components/documents/editor/math/field/mathView/keys";
import type { MathView } from "@/components/documents/editor/math/field/mathView";
import type { FieldCommand } from "@/lib/maths";
import { describe, expect, test } from "bun:test";
import { fieldHtml, unhugMap } from "@/components/documents/editor/math/field/mathView/render";
import { huggedArrays } from "@/components/documents/editor/math/hugArrays";

/** Each mapped range of `html` with the source it covers. */
const ranges = (html: string, source: string) =>
  [...html.matchAll(/data-s="(\d+)" data-e="(\d+)"/g)].map(([, s, e]) => source.slice(Number(s), Number(e)));

describe("the field's rendering", () => {
  test("a hugged array's source map points back into the note's source", () => {
    const source = "\\left[\\begin{array}{cc}p&q\\end{array}\\right]+r";
    expect(huggedArrays(source).at.length).toBe(2);
    const covered = ranges(fieldHtml(source, false), source);
    expect(covered).toContain("p");
    expect(covered).toContain("q");
    expect(covered).toContain("r");
    expect(covered.some((s) => s.includes("kern"))).toBe(false);
  });

  test("a kern's own element loses its range", () => {
    // `x` at 0, a kern inserted at 1 (11 chars), `y` at 1.
    const html = '<a data-s="0" data-e="1"></a><b data-s="1" data-e="12"></b><c data-s="12" data-e="13"></c>';
    expect(unhugMap(html, [1])).toBe('<a data-s="0" data-e="1"></a><b></b><c data-s="1" data-e="2"></c>');
  });
});


test("modifier arrows dispatch row and structural movement, including Shift", () => {
  const commands: FieldCommand[] = [];
  const view = { field: { mode: "math" }, host: {}, run: (command: FieldCommand) => commands.push(command) } as unknown as MathView;
  for (const [key, mods, expected] of [
    ["ArrowLeft", { altKey: true }, { wordLeft: { extend: false } }],
    ["ArrowRight", { altKey: true, shiftKey: true }, { wordRight: { extend: true } }],
    ["ArrowLeft", { metaKey: true }, { home: { extend: false } }],
    ["ArrowRight", { metaKey: true, shiftKey: true }, { end: { extend: true } }],
  ] as const) {
    let prevented = false;
    keyDown(view, { key, ...mods, shiftKey: "shiftKey" in mods && mods.shiftKey, preventDefault: () => { prevented = true; }, stopPropagation() {} } as KeyboardEvent);
    expect(commands.at(-1)).toEqual(expected);
    expect(prevented).toBe(true);
  }
});

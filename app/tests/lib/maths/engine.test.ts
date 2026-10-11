import { describe, expect, test } from "bun:test";
import { MathsTrap, mathsReady, parseError, renderToString } from "@/lib/maths";
import { deep, ready } from "./helpers";

function thrown(fn: () => unknown): Error {
  try {
    fn();
  } catch (e) {
    return e as Error;
  }
  throw new Error("did not throw");
}

describe("maths facade", () => {
  test("renders HTML and MathML with the TeX annotation", () => {
    const html = renderToString("x^2");
    expect(html).toStartWith('<span class="katex">');
    expect(html).toContain('<annotation encoding="application/x-tex">x^2</annotation>');
    expect(renderToString("x", { displayMode: true })).toStartWith('<span class="katex-display">');
  });

  test("macros expand", () => {
    expect(renderToString("\\foo", { macros: { "\\foo": "y" } })).toContain("<mi>y</mi>");
  });

  test("a parse error throws KaTeX's ParseError", () => {
    const e = thrown(() => renderToString("\\frac{x"));
    expect(e.name).toBe("ParseError");
    expect(e.message).toStartWith("KaTeX parse error: ");
    expect(String(e)).toStartWith("ParseError: KaTeX parse error: ");
  });

  test("throwOnError false draws KaTeX's error span", () => {
    const html = renderToString("\\frac{x", { throwOnError: false });
    expect(html).toStartWith('<span class="katex-error" title="ParseError: KaTeX parse error: ');
    expect(html).toEndWith('style="color:#cc0000">\\frac{x</span>');
  });

  test("parseError reports the message, and strict decides", () => {
    expect(parseError("x^2")).toBeUndefined();
    expect(parseError("\\frac{x")).toStartWith("KaTeX parse error: ");
    expect(parseError("é", { strict: "error" })).toContain("unicodeTextInMathMode");
    expect(parseError("é", { strict: "ignore" })).toBeUndefined();
  });

  test("a trap is a render error and the next render works", () => {
    const e = thrown(() => renderToString(deep(300)));
    expect(e).toBeInstanceOf(MathsTrap);
    expect(e.name).toBe("MathsTrap");
    expect(mathsReady()).toBe(true);
    expect(renderToString("x")).toStartWith('<span class="katex">');
    // Remembered: the same formula fails again without trapping.
    expect(thrown(() => renderToString(deep(300), { throwOnError: false }))).toBe(e);
    expect(parseError(deep(300))).toBe(e.message);
  });

  test("two traps in a row leave it pending until a fresh instance is up", async () => {
    await ready();
    expect(() => renderToString(deep(310))).toThrow(MathsTrap);
    expect(() => renderToString(deep(311))).toThrow(MathsTrap);
    expect(mathsReady()).toBe(false);
    await ready();
    expect(renderToString("x")).toStartWith('<span class="katex">');
  });
});

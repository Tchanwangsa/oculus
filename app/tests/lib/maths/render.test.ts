import { beforeEach, describe, expect, test } from "bun:test";
import { renderToString } from "@/lib/maths";
import { renderCacheKey } from "@/lib/maths/render";
import { ready } from "./helpers";

function thrown(fn: () => unknown): Error {
  try {
    fn();
  } catch (e) {
    return e as Error;
  }
  throw new Error("did not throw");
}

describe("renderToString's cache", () => {
  beforeEach(ready);

  test("the same input renders the same string", () => {
    const options = { displayMode: true, sourceMap: true };
    expect(renderToString("\\frac{a}{b}", options)).toBe(renderToString("\\frac{a}{b}", { ...options }));
  });

  test("options equal to the defaults share a key", () => {
    const key = renderCacheKey("x", undefined);
    expect(key).not.toBeNull();
    for (const options of [
      {},
      { throwOnError: true },
      { displayMode: false, sourceMap: false, strict: "warn", output: "htmlAndMathml", macros: {} },
      { displayMode: undefined, macros: undefined },
    ] as const) {
      expect(renderCacheKey("x", options)).toBe(key);
    }
    expect(renderCacheKey("x", { displayMode: true })).not.toBe(key);
    expect(renderCacheKey("x", { strict: true })).not.toBe(renderCacheKey("x", { strict: "error" }));
  });

  test("macros are part of the key, whatever their order", () => {
    const a = { macros: { "\\foo": "a", "\\bar": "b" } };
    const b = { macros: { "\\bar": "b", "\\foo": "a" } };
    expect(renderCacheKey("\\foo", a)).toBe(renderCacheKey("\\foo", b));
    expect(renderToString("\\foo\\bar", a)).toBe(renderToString("\\foo\\bar", b));
    const other = { macros: { "\\foo": "c", "\\bar": "b" } };
    expect(renderCacheKey("\\foo", other)).not.toBe(renderCacheKey("\\foo", a));
    expect(renderToString("\\foo", other)).not.toBe(renderToString("\\foo", a));
    expect(renderToString("\\foo", other)).toContain(">c<");
  });

  test("a parse error is rethrown from the cache", () => {
    const first = thrown(() => renderToString("\\frac{q"));
    expect(first.name).toBe("ParseError");
    expect(thrown(() => renderToString("\\frac{q", { throwOnError: true }))).toBe(first);
    // With throwOnError off it is a different render.
    expect(renderToString("\\frac{q", { throwOnError: false })).toContain("katex-error");
  });

  test("a malformed option is a TypeError every time, never cached", () => {
    const options = { displayMode: "yes" } as never;
    expect(renderCacheKey("x", options)).toBeNull();
    expect(renderCacheKey("x", { macros: { "\\a": 1 } } as never)).toBeNull();
    expect(renderCacheKey("x", { strict: "toString" } as never)).toBeNull();
    const first = thrown(() => renderToString("x", options));
    const second = thrown(() => renderToString("x", options));
    expect(first.name).toBe("TypeError");
    expect(second.name).toBe("TypeError");
    expect(second).not.toBe(first);
  });
});

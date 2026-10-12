import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";

import {
  THEME_FONTS,
  copyName,
  formatTheme,
  parseThemeText,
  sameTokens,
  sectionKeys,
  setFontRole,
  setToken,
  tokenAt,
  typeRole,
  validThemeName,
  type Tokens,
} from "@/lib/studio/themeDraft";

const studioCore = join(import.meta.dir, "../../../studio-core");
const bundled = (): Tokens => JSON.parse(readFileSync(join(studioCore, "themes/default.json"), "utf8"));

describe("tokens", () => {
  test("tokenAt reads a path and is undefined past a leaf", () => {
    const t = bundled();
    expect(tokenAt(t, ["color", "accent"])).toBe("#5e6ad2");
    expect(tokenAt(t, ["type", "h1", "fontSize"])).toBe(72);
    expect(tokenAt(t, ["color", "accent", "x"])).toBeUndefined();
    expect(tokenAt(null, ["color"])).toBeUndefined();
  });

  test("setToken copies, keeps key order, makes missing objects and removes on undefined", () => {
    const t = bundled();
    const next = setToken(t, ["color", "bg"], "#000000");
    expect(tokenAt(next, ["color", "bg"])).toBe("#000000");
    expect(tokenAt(t, ["color", "bg"])).toBe("#f4f4f6");
    expect(Object.keys(next)).toEqual(Object.keys(t));
    expect(sectionKeys(next, "color")).toEqual(sectionKeys(t, "color"));
    expect(next.font).toBe(t.font);
    const made = setToken({}, ["motion", "transition", "kind"], "push");
    expect(made).toEqual({ motion: { transition: { kind: "push" } } });
    const gone = setToken(t, ["motion", "magicMove"], undefined);
    expect("magicMove" in (gone.motion as Tokens)).toBe(false);
  });

  test("sectionKeys reads dotted sections in file order", () => {
    const t = bundled();
    expect(sectionKeys(t, "motion.spring")).toEqual(["gentle", "snappy", "bouncy"]);
    expect(sectionKeys(t, "radius")).toEqual(["s", "m", "l", "xl", "pill"]);
    expect(sectionKeys(t, "nope")).toEqual([]);
  });

  test("formatTheme is two-space JSON with a final newline, and parses back the same", () => {
    const t = bundled();
    const text = formatTheme(t);
    expect(text.endsWith("}\n")).toBe(true);
    expect(text).toContain('\n  "color": {\n    "bg": "#f4f4f6",');
    const back = parseThemeText(text);
    expect("tokens" in back && sameTokens(back.tokens, t)).toBe(true);
  });

  test("parseThemeText names what is wrong", () => {
    expect("error" in parseThemeText("{ nope")).toBe(true);
    expect(parseThemeText("[1]")).toEqual({ error: "A theme file is a JSON object." });
  });

  test("sameTokens notices a change and a reorder", () => {
    const t = bundled();
    expect(sameTokens(t, bundled())).toBe(true);
    expect(sameTokens(t, setToken(t, ["radius", "s"], 9))).toBe(false);
    expect(sameTokens({ a: 1, b: 2 }, { b: 2, a: 1 })).toBe(false);
  });
});

describe("names", () => {
  test("validThemeName is studio-core's rule", () => {
    for (const ok of ["default", "warm", "chalk-2", "a"]) expect(validThemeName(ok)).toBe(true);
    for (const bad of ["", "Chalk", "a_b", "a b", "../x", "x.json", "a".repeat(65)]) expect(validThemeName(bad)).toBe(false);
  });

  test("copyName finds a free -copy name and makes the base a theme name", () => {
    expect(copyName("warm", ["warm"])).toBe("warm-copy");
    expect(copyName("warm", ["warm", "warm-copy", "warm-copy-2"])).toBe("warm-copy-3");
    expect(copyName("My Theme!", [])).toBe("my-theme-copy");
    expect(copyName("___", [])).toBe("theme-copy");
    expect(validThemeName(copyName("x".repeat(80), []))).toBe(true);
  });
});

describe("fonts", () => {
  test("THEME_FONTS are the families studio-core bundles", () => {
    const fonts = readFileSync(join(studioCore, "src/fonts/mod.rs"), "utf8");
    const list = /const BUNDLED[^=]*=\s*&\[([\s\S]*?)\];/.exec(fonts)?.[1] ?? "";
    const names = [...list.matchAll(/\("([^"]+)"/g)].map((m) => m[1]);
    const mono = /pub const MONO_FAMILY: &str = "([^"]+)"/.exec(fonts)?.[1];
    if (list.includes("MONO_FAMILY") && mono) names.push(mono);
    expect([...names].sort()).toEqual([...THEME_FONTS].sort());
  });

  test("typeRole sorts type entries by name", () => {
    expect(["display", "h1", "h3", "title"].map(typeRole)).toEqual(["heading", "heading", "heading", "heading"]);
    expect(["body", "label", "caption"].map(typeRole)).toEqual(["body", "body", "body"]);
    expect(typeRole("code")).toBe("mono");
  });

  test("a new heading font moves the type entries that used the old one", () => {
    const t = bundled();
    const next = setFontRole(t, "heading", "Inter");
    expect(tokenAt(next, ["font", "heading"])).toBe("Inter");
    for (const k of ["display", "h1", "h2", "h3"]) expect(tokenAt(next, ["type", k, "fontFamily"])).toBe("Inter");
    expect(tokenAt(next, ["type", "code", "fontFamily"])).toBe("JetBrains Mono");
    expect(tokenAt(t, ["type", "h1", "fontFamily"])).toBe("Manrope");
  });

  test("when two roles share a family, the entry's name decides", () => {
    const shared = setFontRole(bundled(), "heading", "Inter");
    const next = setFontRole(shared, "body", "Manrope");
    expect(tokenAt(next, ["type", "body", "fontFamily"])).toBe("Manrope");
    expect(tokenAt(next, ["type", "label", "fontFamily"])).toBe("Manrope");
    expect(tokenAt(next, ["type", "h1", "fontFamily"])).toBe("Inter");
    expect(sameTokens(setFontRole(next, "body", "Manrope"), next)).toBe(true);
  });
});

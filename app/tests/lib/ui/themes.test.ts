import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import {
  BUILTIN,
  FRAME_ALPHA,
  MONO_STACK,
  SYNTAX_SLOTS,
  THEMES,
  appearanceTokens,
  contrast,
  defaultAppearance,
  derive,
  getTheme,
  readAppearance,
  themeTokens,
  themesFor,
  validateAppearance,
  type ThemeSpec,
} from "@/lib/ui/themes";

/** `--color-*` declarations in a stylesheet, name to value. */
function tokensIn(file: string): Record<string, string> {
  const css = readFileSync(new URL(`../../../src/styles/${file}`, import.meta.url), "utf8");
  return Object.fromEntries([...css.matchAll(/(--color-[a-z0-9-]+):\s*([^;]+);/g)].map((m) => [m[1], m[2].trim()]));
}

const LIGHT_CSS = tokensIn("colors.css");
const DARK_CSS = tokensIn("dark.css");
/** What a theme owns: chart, reference glyph and task-tag slots stay the
 *  stylesheet's, the indigo ramp is the named indigo (nothing reads it), and
 *  the frame is set by the prefs. */
const THEME_TOKENS = Object.keys(LIGHT_CSS).filter(
  (name) => !/^--color-(chart-|reference-|tag-|indigo-|frame$)/.test(name),
);
const RESOLVED = /^#[0-9a-f]{6}$/;

describe.each(THEMES.map((t) => [t.id, t] as const))("%s", (_id, theme: ThemeSpec) => {
  const tokens = derive(theme);
  const on = (fg: string, bg: string) => contrast(tokens[fg], tokens[bg]);

  test("derives every token the stylesheet defines, each a resolved colour", () => {
    expect(Object.keys(tokens).sort()).toEqual([...THEME_TOKENS].sort());
    for (const [name, value] of Object.entries(tokens)) expect([name, value]).toEqual([name, expect.stringMatching(RESOLVED)]);
  });

  test("text reads on the card and on fills", () => {
    expect(on("--color-foreground", "--color-card")).toBeGreaterThanOrEqual(7);
    expect(on("--color-muted-foreground", "--color-card")).toBeGreaterThanOrEqual(4.5);
    expect(on("--color-primary-foreground", "--color-primary")).toBeGreaterThanOrEqual(4.5);
  });

  test("status colours and the brand stand off the card", () => {
    for (const name of ["destructive", "success", "warning", "brand"]) {
      expect({ name, ok: on(`--color-${name}`, "--color-card") >= 3 }).toEqual({ name, ok: true });
    }
  });

  test("every syntax colour reads on surface-raised", () => {
    for (const slot of SYNTAX_SLOTS) {
      const ratio = contrast(theme.syntax[slot], tokens["--color-surface-raised"]);
      expect({ slot, ok: ratio >= 3 }).toEqual({ slot, ok: true });
    }
  });

  test("the ground sits one step off the card", () => {
    expect(theme.palette.ground).not.toBe(theme.palette.card);
  });
});

describe("built-ins", () => {
  const cases = [
    [BUILTIN.light, LIGHT_CSS],
    [BUILTIN.dark, { ...LIGHT_CSS, ...DARK_CSS }],
  ] as const;

  test("apply nothing inline", () => {
    for (const [theme] of cases) expect(themeTokens(theme)).toEqual({});
  });

  test("carry the stylesheet's palette and syntax, for previews", () => {
    for (const [{ palette: p, syntax }, css] of cases) {
      expect({
        ground: p.ground,
        card: p.card,
        popover: p.popover,
        foreground: p.foreground,
        mutedForeground: p.mutedForeground,
        border: p.border,
        accent: p.accent,
        destructive: p.destructive,
        success: p.success,
        warning: p.warning,
      }).toEqual({
        ground: css["--color-background"],
        card: css["--color-card"],
        popover: css["--color-popover"],
        foreground: css["--color-foreground"],
        mutedForeground: css["--color-muted-foreground"],
        border: css["--color-border"],
        accent: css["--color-primary"],
        destructive: css["--color-destructive"],
        success: css["--color-success"],
        warning: css["--color-warning"],
      });
      for (const slot of SYNTAX_SLOTS) expect(syntax[slot]).toBe(css[`--color-syntax-${slot}`]);
    }
  });
});

describe("registry", () => {
  test("ids are unique and each mode lists its built-in first", () => {
    expect(new Set(THEMES.map((t) => t.id)).size).toBe(THEMES.length);
    expect(themesFor("light")[0]).toBe(BUILTIN.light);
    expect(themesFor("dark")[0]).toBe(BUILTIN.dark);
    expect(themesFor("dark").every((t) => t.mode === "dark")).toBe(true);
  });

  test("an unknown id, or one from the other mode, falls back to the built-in", () => {
    expect(getTheme("nord", "dark").id).toBe("nord");
    expect(getTheme("nord", "light")).toBe(BUILTIN.light);
    expect(getTheme("gone", "dark")).toBe(BUILTIN.dark);
    expect(getTheme(null, "light")).toBe(BUILTIN.light);
  });
});

describe("appearance prefs", () => {
  test("defaults: light, built-ins, frosted only on macOS", () => {
    expect(defaultAppearance(true)).toEqual({
      mode: "light",
      light: "oculus-light",
      dark: "oculus-dark",
      accent: null,
      frame: "frosted",
      codeFont: "",
    });
    expect(defaultAppearance(false).frame).toBe("solid");
  });

  test("the old theme key becomes the mode when nothing new is stored", () => {
    expect(readAppearance(null, "dark", true)).toEqual({ ...defaultAppearance(true), mode: "dark" });
    expect(readAppearance(null, "system", false).mode).toBe("system");
    expect(readAppearance(null, "sepia", true).mode).toBe("light");
    expect(readAppearance("{not json", "dark", true).mode).toBe("dark");
    // Once the new key exists the old one is ignored.
    expect(readAppearance(JSON.stringify({ mode: "light" }), "dark", true).mode).toBe("light");
  });

  test("round-trips through JSON", () => {
    const prefs = validateAppearance(
      {
        mode: "system",
        light: "paper",
        dark: "nord",
        accent: "#E5484D",
        frame: "clear",
        codeFont: "JetBrains Mono",
      },
      true,
    );
    expect(prefs.accent).toBe("#e5484d");
    expect(readAppearance(JSON.stringify(prefs), null, true)).toEqual(prefs);
  });

  test("corrects what does not validate", () => {
    const prefs = validateAppearance(
      {
        mode: "dim",
        light: "nord",
        dark: "missing",
        accent: "#12345678",
        frame: "glass",
        codeFont: 'Fira"; color: red',
      },
      true,
    );
    expect(prefs).toEqual(defaultAppearance(true));
    expect(validateAppearance({ accent: "blue" }, true).accent).toBeNull();
    expect(validateAppearance({ frame: "clear" }, false).frame).toBe("solid");
    expect(validateAppearance(null, true)).toEqual(defaultAppearance(true));
  });

  test("drops keys it does not know, such as the old per-mode code themes", () => {
    const stored = JSON.stringify({ mode: "dark", codeLight: "github-light", codeDark: "dracula" });
    expect(readAppearance(stored, null, true)).toEqual({ ...defaultAppearance(true), mode: "dark" });
  });
});

describe("appearanceTokens", () => {
  const solid = { ...defaultAppearance(false) };

  test("the built-in with a solid frame writes nothing", () => {
    expect(appearanceTokens(solid, "light")).toEqual({});
    expect(appearanceTokens(solid, "dark")).toEqual({});
  });

  test("a see-through frame writes only the ground at the frame's alpha", () => {
    const tokens = appearanceTokens({ ...solid, frame: "frosted" }, "dark");
    expect(Object.keys(tokens)).toEqual(["--color-frame"]);
    expect(tokens["--color-frame"]).toBe(`#171717${Math.round(FRAME_ALPHA.frosted * 255).toString(16)}`);
  });

  test("a sample theme writes its derived tokens for the resolved mode only", () => {
    const prefs = { ...solid, light: "paper", dark: "nord" };
    expect(appearanceTokens(prefs, "dark")).toEqual(derive(getTheme("nord", "dark")));
    expect(appearanceTokens(prefs, "light")["--color-card"]).toBe(getTheme("paper", "light").palette.card);
  });

  test("the accent replaces the fill, brand and ring", () => {
    const tokens = appearanceTokens({ ...solid, accent: "#e5484d" }, "light");
    expect(tokens["--color-primary"]).toBe("#e5484d");
    expect(tokens["--color-brand"]).toBe("#e5484d");
    expect(tokens["--color-ring"]).toBe("#e5484d");
    expect(contrast(tokens["--color-primary-foreground"], "#e5484d")).toBeGreaterThanOrEqual(4.5);
    expect(tokens["--color-card"]).toBeUndefined();
  });

  test("the monospace stack is the stylesheet's", () => {
    const css = readFileSync(new URL("../../../src/styles/theme.css", import.meta.url), "utf8");
    expect(css).toContain(`--font-mono: ${MONO_STACK};`);
  });

  test("a code font leads the monospace stack", () => {
    expect(appearanceTokens({ ...solid, codeFont: "JetBrains Mono" }, "light")).toEqual({
      "--font-mono": `"JetBrains Mono", ${MONO_STACK}`,
    });
  });
});

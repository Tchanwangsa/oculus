import { beforeEach, describe, expect, test } from "bun:test";

const store = new Map<string, string>();
(globalThis as { localStorage?: unknown }).localStorage = {
  getItem: (k: string) => store.get(k) ?? null,
  setItem: (k: string, v: string) => void store.set(k, v),
  removeItem: (k: string) => void store.delete(k),
};

const { entryForCommand, popularEntries, quickPicks, readRecents, recordCommand, recordUse } = await import(
  "../src/components/documents/editor/mathUsage"
);

const cells = (list: { wide?: boolean }[]) => list.reduce((n, e) => n + (e.wide ? 2 : 1), 0);

describe("maths toolbox usage", () => {
  beforeEach(() => store.clear());

  test("a typed command counts as its palette entry, bare when there is one", () => {
    expect(entryForCommand("\\frac")?.template).toBe("\\frac{#{}}{#{}}");
    expect(entryForCommand("\\sum")?.template).toBe("\\sum");
    expect(entryForCommand("\\mathbb")?.template).toBe("\\mathbb{#{}}");
    expect(entryForCommand("\\notacommand")).toBeNull();
  });

  test("typed commands join the recents, unknown ones don't", () => {
    recordCommand("\\alpha", 1);
    recordCommand("\\notacommand", 1);
    recordCommand("\\frac", 1);
    expect(readRecents().map((e) => e.template)).toEqual(["\\frac{#{}}{#{}}", "\\alpha"]);
  });

  test("Popular puts the most used first and pads with defaults to three rows", () => {
    const empty = popularEntries();
    expect(empty[0].template).toBe("\\frac{#{}}{#{}}");
    expect(cells(empty)).toBe(24);
    recordUse({ template: "\\omega" }, 1);
    recordCommand("\\theta", 1);
    recordCommand("\\theta", 1);
    const list = popularEntries();
    expect(list.slice(0, 2).map((e) => e.template)).toEqual(["\\theta", "\\omega"]);
    expect(list.filter((e) => e.template === "\\theta")).toHaveLength(1);
    expect(cells(list)).toBe(24);
  });

  test("quick picks are the subject's most recent, padded from Popular", () => {
    const popular = popularEntries().map((e) => e.template);
    expect(quickPicks(1).map((e) => e.template)).toEqual(popular.slice(0, 5));
    recordCommand("\\theta", 1);
    recordCommand("\\omega", 1);
    recordCommand("\\theta", 1);
    recordCommand("\\nabla", 2);
    const picks = quickPicks(1).map((e) => e.template);
    expect(picks.slice(0, 2)).toEqual(["\\theta", "\\omega"]);
    expect(picks).toHaveLength(5);
    expect(new Set(picks).size).toBe(5);
    // Another subject's recents only reach this one as Popular padding.
    expect(picks.indexOf("\\nabla")).not.toBeLessThan(2);
    expect(quickPicks(2).map((e) => e.template).slice(0, 1)).toEqual(["\\nabla"]);
    for (const name of ["\\alpha", "\\beta", "\\gamma", "\\delta", "\\epsilon", "\\zeta"]) recordCommand(name, null);
    expect(quickPicks(null).map((e) => e.template)).toEqual(["\\zeta", "\\epsilon", "\\delta", "\\gamma", "\\beta"]);
  });

  test("the usage map is capped and survives a broken store", () => {
    for (let i = 0; i < 300; i++) recordUse({ template: `x_{${i}}` }, null);
    expect(JSON.parse(store.get("oculus-math-usage")!).length).toBeLessThanOrEqual(120);
    store.set("oculus-math-usage", "{not json");
    expect(cells(popularEntries())).toBe(24);
  });
});

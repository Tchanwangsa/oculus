import { describe, expect, test } from "bun:test";
import { createLibrary, evaluate } from "@openuidev/lang-core";
import { Sample, Means, Bins, Normal, Binomial, Uniform, Exponential, Mean, StdDev, Pow, Sqrt, Log, Range, functions } from "@/components/markdown/openui/functions";

const call = (f: { fn: (args: never) => unknown }, args: unknown) => f.fn(args as never);
const args = { dist: "exponential", params: { rate: 1 }, n: 30, seed: 42 };

describe("pure simulations", () => {
  for (const [f, name] of [[Normal, "normal"], [Binomial, "binomial"], [Uniform, "uniform"], [Exponential, "exponential"]] as const) {
    test(`${f.name} is a distribution specifier, with validated arguments`, () => {
      expect(call(f, {})).toBe(name);
      expect(f.params.safeParse({}).success).toBe(true);
      expect(f.params.safeParse(null).success).toBe(false);
    });
    test(`${name} samples are deterministic, finite, bounded and validate their parameters`, () => {
      const input = { ...args, dist: name, params: {} };
      const samples = call(Sample, input) as number[];
      expect(samples).toHaveLength(30);
      expect(samples).toEqual(call(Sample, input));
      expect(samples).not.toEqual(call(Sample, { ...input, seed: 43 }));
      expect(samples.every(Number.isFinite)).toBe(true);
      if (name !== "normal") expect(samples.every(n => n >= 0)).toBe(true);
      if (name === "uniform" || name === "binomial") expect(samples.every(n => n <= 1)).toBe(true);
      if (name === "binomial") expect(samples.every(Number.isInteger)).toBe(true);
      expect(call(Sample, { ...input, params: { surprise: 1 } })).toBeNull();
      expect(call(Means, { ...input, trials: 20 })).toEqual(call(Means, { ...input, trials: 20 }));
    });
  }
  test("Sample caps output, keeps seed zero deterministic and rejects invalid args", () => {
    expect(call(Sample, { ...args, n: 1e9 })).toHaveLength(400);
    expect(call(Sample, { ...args, seed: 0 })).toEqual(call(Sample, { ...args, seed: 0 }));
    expect(call(Sample, { dist: "uniform", params: { min: 5, max: 5 }, n: 2 })).toEqual([5, 5]);
    for (const patch of [{ n: 0 }, { n: -1 }, { n: 1.5 }, { n: Infinity }, { seed: NaN }, { seed: 1.1 }, { dist: "unknown" }, { params: { rate: 0 } }, { params: { rate: -1 } }, { params: null }]) {
      expect(call(Sample, { ...args, ...patch })).toBeNull();
    }
    for (const [dist, params] of [["normal", { sd: -1 }], ["uniform", { min: 10, max: 0 }], ["binomial", { trials: 401 }], ["binomial", { p: 1.1 }], ["normal", { mean: Infinity }]]) {
      expect(call(Sample, { ...args, dist, params })).toBeNull();
    }
  });
  test("Means caps trials and the work budget, including binomial Bernoulli draws", () => {
    expect(call(Means, { ...args, trials: 1e9 })).toHaveLength(400);
    expect(call(Means, { ...args, n: 500, trials: 400 })).toHaveLength(400);
    for (const patch of [{ n: 501 }, { n: 0 }, { n: 1.5 }, { trials: 0 }, { trials: -1 }, { seed: Infinity }, { params: { rate: 0 } }]) {
      expect(call(Means, { ...args, trials: 400, ...patch })).toBeNull();
    }
    expect(call(Means, { dist: "binomial", params: { trials: 400 }, n: 2, trials: 400 })).toBeNull();
    const one = call(Means, { ...args, n: 1, trials: 30 });
    expect(one).toEqual(call(Sample, args));
  });
  test("Bins conserves counts, includes the upper edge and caps bins", () => {
    expect(call(Bins, { values: [0, 1, 2, 3, 4], count: 2 })).toEqual([{ x: 1, y: 2 }, { x: 3, y: 3 }]);
    const bins = call(Bins, { values: [5, 5], count: 2 }) as { x: number; y: number }[];
    expect(bins.reduce((sum, b) => sum + b.y, 0)).toBe(2);
    expect(bins.every(b => Number.isFinite(b.x))).toBe(true);
    expect(call(Bins, { values: [1, 2], count: 1e9 })).toHaveLength(400);
    expect(call(Bins, { values: [], count: 3 })).toEqual([]);
    for (const input of [{ values: [NaN], count: 2 }, { values: [1], count: 0 }, { values: [1], count: 1.5 }, { values: Array(401).fill(0), count: 2 }]) expect(call(Bins, input)).toBeNull();
  });
  for (const f of [Mean, StdDev]) test(`${f.name}: population statistic, bounds and invalid inputs`, () => {
    expect(call(f, { values: [1, 2, 3] })).toBeCloseTo(f === Mean ? 2 : Math.sqrt(2 / 3));
    expect(call(f, { values: [7] })).toBe(f === Mean ? 7 : 0);
    expect(call(f, { values: Array(400).fill(2) })).toBeCloseTo(f === Mean ? 2 : 0);
    for (const values of [[], [NaN], [Infinity], ["1"], Array(401).fill(1)]) expect(call(f, { values })).toBeNull();
  });
});

test("the maths functions still evaluate and reject invalid inputs as null", () => {
  const schema = createLibrary({ components: [], functions }).toJSONSchema();
  const context = { getState: () => undefined, functions: Object.fromEntries(functions.map(f => [f.name, { ...schema.functions![f.name], fn: f.fn }])) };
  const run = (name: string, ...values: (number | string)[]) => evaluate({ k: "Comp", name, fn: true, args: values.map(v => typeof v === "number" ? { k: "Num", v } : { k: "Str", v }) }, context);
  expect(run(Pow.name, 2, 3)).toBe(8);
  expect(run(Sqrt.name, 9)).toBe(3);
  expect(run(Log.name, 1)).toBe(0);
  expect(run(Range.name, 0, 1e9)).toHaveLength(400);
  for (const f of [Pow, Sqrt, Log, Range]) expect(run(f.name, "invalid")).toBeNull();
  expect(run(Sqrt.name, -1)).toBeNull();
  expect(run(Log.name, 0)).toBeNull();
});

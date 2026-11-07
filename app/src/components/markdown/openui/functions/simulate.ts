import { defineFunction } from "@openuidev/react-lang";
import { z } from "zod/v4";
import { distribution, sampler, MAX_POINTS, MAX_DRAWS } from "./distributions";

const positiveInt = z.number().int().positive().max(Number.MAX_SAFE_INTEGER);
const seed = z.number().int().min(-2147483648).max(4294967295).default(0);
const sampling = { dist: distribution, params: z.record(z.string(), z.number()), n: positiveInt };
const numbers = z.array(z.number()).max(MAX_POINTS);
const points = z.array(z.object({ x: z.number(), y: z.number() }));

/** Domain errors return null without reporting a render error for the fence. */
function pure<S extends z.ZodRawShape, R extends z.ZodType>(name: string, description: string, params: z.ZodObject<S>, returns: R, fn: (args: z.infer<z.ZodObject<S>>) => z.infer<R> | null) {
  return { ...defineFunction({ name, description, params, fn: (args) => {
    const parsed = params.safeParse(args);
    if (!parsed.success) return null;
    const value = fn(parsed.data);
    const result = returns.safeParse(value);
    return result.success ? result.data : null;
  } }), returns: returns.nullable() };
}

function spec(name: string, dist: z.infer<typeof distribution>, description: string) {
  return defineFunction({ name, description: `Distribution name "${dist}" for @Sample/@Means, not a density. ${description} Omitted parameter keys use these defaults.`, params: z.object({}), returns: distribution, fn: () => dist });
}
export const Normal = spec("Normal", "normal", "params {mean:0, sd:1}; sd >= 0.");
export const Binomial = spec("Binomial", "binomial", "params {trials:1, p:0.5}; integer trials 0–400, probability 0–1.");
export const Uniform = spec("Uniform", "uniform", "params {min:0, max:1}; max >= min.");
export const Exponential = spec("Exponential", "exponential", "params {rate:1}; rate > 0.");

export const Sample = pure("Sample", "Seeded draws: @Sample(@Exponential(), {rate:1}, 400, 42). dist may also be a distribution name string; at most 400 points; seed defaults to 0. Invalid parameters return null.",
  z.object({ ...sampling, seed }), numbers, ({ dist, params, n, seed }) => {
    const source = sampler(dist, params, seed);
    if (!source) return null;
    return Array.from({ length: Math.min(n, MAX_POINTS) }, source.draw);
  });

export const Means = pure("Means", "Seeded sample means: @Means(@Exponential(), {}, $n, 400, 42). At most 400 trials; trials × n <= 200000 (also capped at 200000 Bernoulli draws for Binomial); over-budget/invalid inputs return null. Seed defaults to 0.",
  z.object({ ...sampling, trials: positiveInt, seed }), numbers, ({ dist, params, n, trials, seed }) => {
    const source = sampler(dist, params, seed);
    const count = Math.min(trials, MAX_POINTS);
    if (!source || n > MAX_DRAWS / count / source.cost) return null;
    return Array.from({ length: count }, () => {
      let mean = 0;
      for (let i = 0; i < n; i++) mean += source.draw() / n;
      return mean;
    });
  });

export const Bins = pure("Bins", "Histogram {x:bin centre, y:count} points for Chart(..., ..., ..., \"bar\"). @Bins(values, 20); count capped at 400; values at most 400. Includes the maximum in the last bin; constant values use a unit-wide domain; empty input returns [].",
  z.object({ values: numbers, count: positiveInt }), points, ({ values, count }) => {
    if (!values.length) return [];
    const min = Math.min(...values), max = Math.max(...values);
    const size = Math.min(count, MAX_POINTS);
    const low = min === max ? min - 0.5 : min;
    const width = ((min === max ? max + 0.5 : max) - low) / size;
    if (!Number.isFinite(width) || width <= 0) return null;
    const bins = Array.from({ length: size }, (_, i) => ({ x: low + (i + 0.5) * width, y: 0 }));
    for (const value of values) bins[Math.min(size - 1, Math.max(0, Math.floor((value - low) / width)))].y++;
    return bins;
  });

export const Mean = pure("Mean", "Arithmetic mean of 1–400 finite numbers; empty or invalid input returns null.",
  z.object({ values: numbers.min(1) }), z.number(), ({ values }) => values.reduce((mean, v) => mean + v / values.length, 0));

export const StdDev = pure("StdDev", "Population standard deviation of 1–400 finite numbers (divisor N); one value gives 0; empty or invalid input returns null.",
  z.object({ values: numbers.min(1) }), z.number(), ({ values }) => {
    const mean = values.reduce((sum, v) => sum + v / values.length, 0);
    return Math.hypot(...values.map(v => (v - mean) / Math.sqrt(values.length)));
  });

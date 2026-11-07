import { z } from "zod/v4";

export const distribution = z.enum(["normal", "binomial", "uniform", "exponential"]);
export const MAX_POINTS = 400;
export const MAX_DRAWS = 200_000;

/** Mulberry32, local to one evaluation; never consumes ambient randomness. */
export function random(seed: number) {
  let state = seed >>> 0;
  return () => {
    state = (state + 0x6d2b79f5) >>> 0;
    let t = Math.imul(state ^ (state >>> 15), state | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

const schemas = {
  normal: z.object({ mean: z.number().default(0), sd: z.number().nonnegative().default(1) }).strict(),
  uniform: z.object({ min: z.number().default(0), max: z.number().default(1) }).strict(),
  exponential: z.object({ rate: z.number().positive().default(1) }).strict(),
  binomial: z.object({ trials: z.number().int().min(0).max(MAX_POINTS).default(1), p: z.number().min(0).max(1).default(0.5) }).strict(),
};

export function sampler(dist: z.infer<typeof distribution>, params: Record<string, number>, seed: number) {
  const rng = random(seed);
  switch (dist) {
    case "normal": {
      const parsed = schemas.normal.safeParse(params);
      if (!parsed.success) return null;
      const { mean, sd } = parsed.data;
      return { cost: 1, draw: () => mean + sd * Math.sqrt(-2 * Math.log(1 - rng())) * Math.cos(2 * Math.PI * rng()) };
    }
    case "uniform": {
      const parsed = schemas.uniform.safeParse(params);
      if (!parsed.success || parsed.data.max < parsed.data.min) return null;
      const { min, max } = parsed.data;
      return { cost: 1, draw: () => { const u = rng(); return min * (1 - u) + max * u; } };
    }
    case "exponential": {
      const parsed = schemas.exponential.safeParse(params);
      if (!parsed.success) return null;
      return { cost: 1, draw: () => -Math.log(1 - rng()) / parsed.data.rate };
    }
    case "binomial": {
      const parsed = schemas.binomial.safeParse(params);
      if (!parsed.success) return null;
      const { trials, p } = parsed.data;
      return { cost: Math.max(1, trials), draw: () => {
        let successes = 0;
        for (let i = 0; i < trials; i++) successes += Number(rng() < p);
        return successes;
      } };
    }
  }
}

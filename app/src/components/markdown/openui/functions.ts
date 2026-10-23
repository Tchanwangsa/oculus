import { defineFunction } from "@openuidev/react-lang";
import { z } from "zod/v4";

/**
 * The @functions a program may call beyond the built-ins (@Round, @Sum, @Each…).
 * Arithmetic in OpenUI Lang has no power operator and no way to build a
 * series, which a formula explorer needs. A bad argument or a throw makes the
 * call evaluate to null (`finite` throws on NaN and ±∞), so a readout
 * shows a dash rather than NaN.
 */

const finite = (n: number) => {
  if (!Number.isFinite(n)) throw new Error("not a finite number");
  return n;
};

export const Pow = defineFunction({
  name: "Pow",
  description: "base raised to exponent, e.g. @Pow(1.05, 10)",
  params: z.object({ base: z.number(), exponent: z.number() }),
  returns: z.number(),
  fn: ({ base, exponent }) => finite(Math.pow(base, exponent)),
});

export const Sqrt = defineFunction({
  name: "Sqrt",
  description: "Square root; null for a negative number",
  params: z.object({ number: z.number() }),
  returns: z.number(),
  fn: ({ number }) => finite(Math.sqrt(number)),
});

export const Log = defineFunction({
  name: "Log",
  description: "Natural logarithm; null for zero or less",
  params: z.object({ number: z.number() }),
  returns: z.number(),
  fn: ({ number }) => finite(Math.log(number)),
});

/** Capped so a slider dragged to an absurd bound cannot freeze the page. */
const MAX_POINTS = 400;

export const Range = defineFunction({
  name: "Range",
  description:
    "Numbers from start to end inclusive, stepping by step (default 1), e.g. @Range(0, 10) for a chart's x values; at most 400",
  params: z.object({ start: z.number(), end: z.number(), step: z.number().optional() }),
  returns: z.array(z.number()),
  fn: ({ start, end, step }) => {
    const by = step && step > 0 ? step : 1;
    const out: number[] = [];
    for (let x = start; x <= end + 1e-9 && out.length < MAX_POINTS; x += by) out.push(Number(x.toFixed(10)));
    return out;
  },
});

export const functions = [Pow, Sqrt, Log, Range];

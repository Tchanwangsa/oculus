import { mathsReady, onMathsReady } from "@/lib/maths";

/** Nested past the engine's stack: every render of it traps. `n` keeps
 *  formulas apart, since a formula that trapped once fails without a call. */
export const deep = (n: number) => "\\frac{".repeat(n) + "x" + "}{y}".repeat(n);

/** Resolves once the engine is up again. */
export function ready(): Promise<void> {
  if (mathsReady()) return Promise.resolve();
  return new Promise((resolve) => {
    const off = onMathsReady(() => {
      if (!mathsReady()) return;
      off();
      resolve();
    });
  });
}

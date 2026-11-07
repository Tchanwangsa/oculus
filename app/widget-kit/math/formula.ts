import type { MathNode } from "mathjs";
import { CONSTANTS, FUNCTIONS, math } from "./instance";

export type Formula =
  | {
      ok: true;
      /** The formula as a function of the variables, in their order. A value
       *  that isn't a real number (complex, a matrix, a domain error) is `NaN`,
       *  which a plot skips. */
      fn: (...values: number[]) => number;
      /** The formula as TeX, for `Tex`. */
      tex: string;
    }
  | { ok: false; error: string };

const isSymbol = (node: MathNode): node is MathNode & { name: string } => node.type === "SymbolNode";

/** The first name in `node` that is neither a variable, a constant nor a
 *  function the instance has, as the error to show. */
function unknownName(node: MathNode, variables: Set<string>): string | null {
  const called = new Set<MathNode>();
  for (const n of node.filter((n) => n.type === "FunctionNode")) {
    const fn = (n as MathNode & { fn: MathNode }).fn;
    called.add(fn);
    if (isSymbol(fn) && !FUNCTIONS.has(fn.name)) return `"${fn.name}" isn't a function here`;
  }
  for (const n of node.filter(isSymbol)) {
    if (called.has(n) || !isSymbol(n)) continue;
    if (!variables.has(n.name) && !CONSTANTS.has(n.name)) {
      const names = [...variables].join(", ");
      return `"${n.name}" isn't defined${names ? ` (the variables are ${names})` : ""}`;
    }
  }
  return null;
}

/** A result as a real number: a complex one with no imaginary part counts. */
function real(value: unknown): number {
  if (typeof value === "number") return value;
  if (typeof value === "boolean") return value ? 1 : 0;
  const z = value as { re?: unknown; im?: unknown } | null;
  if (z && typeof z.re === "number" && typeof z.im === "number") return Math.abs(z.im) < 1e-12 ? z.re : NaN;
  return NaN;
}

/**
 * Reads `expression` (mathjs syntax: `x^2 + 3x`, `sin(t)/t`) as a function
 * of `variables` (default `["x"]`). Never throws: a formula that doesn't
 * parse, or names something unknown, comes back as `{ ok: false, error }`
 * for the widget to show while the student is still typing.
 */
export function formula(expression: string, variables: string[] = ["x"]): Formula {
  const text = expression.trim();
  if (!text) return { ok: false, error: "Type a formula" };
  let node: MathNode;
  try {
    node = math.parse(text);
  } catch (error) {
    return { ok: false, error: `Couldn't read "${text}": ${error instanceof Error ? error.message : String(error)}` };
  }
  const unknown = unknownName(node, new Set(variables));
  if (unknown) return { ok: false, error: unknown };
  const code = node.compile();
  const scope: Record<string, number> = {};
  const fn = (...values: number[]) => {
    variables.forEach((name, i) => (scope[name] = values[i]));
    try {
      return real(code.evaluate(scope));
    } catch {
      return NaN;
    }
  };
  return { ok: true, fn, tex: node.toTex() };
}

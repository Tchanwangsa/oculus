import { createParser, type ParseResult } from "@openuidev/react-lang";
import { library } from "./library";
import { JOB_KINDS, PROPOSAL_KINDS, validateCardTree } from "./proposals/model";

/** One parser for the app's lifetime: it is built from the library's JSON
 *  Schema, which `toJSONSchema` rebuilds on every call. */
let parser: ReturnType<typeof createParser> | null = null;

export type Verdict =
  | { ok: true; result: ParseResult; card: boolean }
  | { ok: false; reason: "empty" | "incomplete" | "invalid"; errors: string[]; result: ParseResult | null };

/**
 * Whether a fence is a program worth drawing. The parser is permissive — it
 * drops what it can't read and renders the rest — so a widget with a
 * component missing would draw half a quiz. Here any error, a truncated
 * input or a dangling reference refuses the whole fence, and the reply shows
 * a failure card instead.
 */
export function parseProgram(source: string): Verdict {
  parser ??= createParser(library.toJSONSchema(), library.root);
  let result: ParseResult;
  try {
    result = parser.parse(source);
  } catch (e) {
    return { ok: false, reason: "invalid", errors: [String(e)], result: null };
  }
  if (result.mutationStatements.length) return { ok: false, reason: "invalid", errors: ["Mutation is not supported; use a proposal for writes"], result };
  const errors = result.meta.errors.filter((e) => e.severity !== "warning").map((e) => `${e.code}: ${e.message}`);
  if (errors.length) return { ok: false, reason: "invalid", errors, result };
  if (result.meta.incomplete || result.meta.unresolved.length)
    return {
      ok: false,
      reason: "incomplete",
      errors: result.meta.unresolved.map((n) => `unresolved: ${n}`),
      result,
    };
  if (!result.root && !result.rootExpression) return { ok: false, reason: "empty", errors: [], result };
  let card: boolean;
  try { card = validateCardTree(result.root); }
  catch (error) { return { ok: false, reason: "invalid", errors: [String(error)], result }; }
  return { ok: true, result, card };
}

/** A partial root is drawable despite incomplete arguments and references.
 * The runtime owns streaming errors; card validation belongs to the close. */
export function streamVerdict(source: string): { drawable: boolean; card: boolean; pending: boolean; result: ParseResult | null } {
  parser ??= createParser(library.toJSONSchema(), library.root);
  try {
    const result = parser.parse(source);
    const items = result.root?.props.items;
    const nodes = Array.isArray(items) ? items.filter((item) => item && typeof item === "object" && "typeName" in item) : [];
    const card = nodes.some((item) => PROPOSAL_KINDS.has(item.typeName) || JOB_KINDS.has(item.typeName));
    return { drawable: nodes.length > 0, card,
      pending: result.meta.incomplete || result.meta.unresolved.length > 0 || result.meta.errors.some((error) => error.severity !== "warning"), result };
  } catch {
    return { drawable: false, card: false, pending: true, result: null };
  }
}

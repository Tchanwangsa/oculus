import { createParser, type ParseResult } from "@openuidev/react-lang";
import { library } from "./library";
import { validateCardTree } from "./proposals/model";

/** One parser for the app's lifetime: it is built from the library's JSON
 *  Schema, which `toJSONSchema` rebuilds on every call. */
let parser: ReturnType<typeof createParser> | null = null;

export type Verdict =
  | { ok: true; result: ParseResult }
  | { ok: false; reason: "empty" | "incomplete" | "invalid"; errors: string[]; result: ParseResult | null };

/**
 * Whether a fence is a program worth drawing. The parser is permissive — it
 * drops what it can't read and renders the rest — so a widget with a
 * component missing would draw half a quiz. Here any error, a truncated
 * input or a dangling reference refuses the whole fence, and the reply shows
 * the code block instead.
 */
export function parseProgram(source: string): Verdict {
  parser ??= createParser(library.toJSONSchema(), library.root);
  let result: ParseResult;
  try {
    result = parser.parse(source);
  } catch (e) {
    return { ok: false, reason: "invalid", errors: [String(e)], result: null };
  }
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
  try { validateCardTree(result.root); }
  catch (error) { return { ok: false, reason: "invalid", errors: [String(error)], result }; }
  return { ok: true, result };
}

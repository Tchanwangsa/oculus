import { DEFAULT_BINDING } from "../../../../../widget-kit/protocol";
import { closePrefix } from "./repair";

/** A default export, the one thing a prefix must have to be worth a transpile. */
const HAS_DEFAULT = /\bexport\s+default\b|\bas\s+default\b/;

/**
 * What to try drawing from a streaming fence's `source`: its complete lines,
 * repaired to parse (`closePrefix`). Null when there is nothing to try: no
 * whole line yet, no default export yet (the transpile would refuse it), or
 * a cut inside a loop's body.
 */
export function previewSource(source: string): string | null {
  const end = source.lastIndexOf("\n");
  if (end < 0) return null;
  const lines = source.slice(0, end);
  if (!HAS_DEFAULT.test(lines)) return null;
  return closePrefix(lines);
}

/** A JS identifier, which a pending name must be before it is declared. */
const IDENT = /^[A-Za-z_$][\w$]*$/;

export interface ModuleInput {
  /** The transpiled widget (`widget_transpile`). */
  js: string;
  /** Components it draws that aren't written yet (`widget_transpile`'s `pending`). */
  pending: readonly string[];
  /** This update's number in its frame, increasing. */
  id: number;
  /** The committed program: no placeholders, no timer scope, errors reported. */
  final: boolean;
  /** The kit's `runtime.js` URL. */
  runtime: string;
}

/**
 * The module a streaming frame runs for one update (`widget-kit/runtime/stream.tsx`).
 * A preview declares each pending component as the kit's placeholder and
 * shadows the timer globals with ones scoped to this update, so the next
 * update cancels whatever this one left running; then it hands its component
 * to `update`. The final program is the module as transpiled, opened by a
 * `commit` that ends the stream before any of its own code runs.
 */
export function moduleCode({ js, pending, id, final, runtime }: ModuleInput): string {
  const from = JSON.stringify(runtime);
  const head = final
    ? `import { commit as __oculusCommit } from ${from};\n__oculusCommit(${id});\n`
    : [
        `import { pendingPart as __oculusPending, previewScope as __oculusScope } from ${from};`,
        `const { setTimeout, clearTimeout, setInterval, clearInterval, requestAnimationFrame, cancelAnimationFrame } = __oculusScope(${id});`,
        ...pending.filter((name) => IDENT.test(name)).map((name) => `const ${name} = __oculusPending(${JSON.stringify(name)});`),
        "",
      ].join("\n");
  return `${head}${js}\nimport { update as __oculusUpdate } from ${from};\n__oculusUpdate(${DEFAULT_BINDING}, ${id});\n`;
}

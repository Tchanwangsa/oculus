/** LaTeX as the note keeps it: placeholders already gone, `\operatorname`
 *  without MathLive's inner `\mathrm`, and nothing that would end inline
 *  maths early (`x\ $` reads as an escaped space before the `$`). */
export function tidy(latex: string, inline: boolean): string {
  const out = latex
    .replace(/\\operatorname(\*?)\{\\mathrm\{([^{}]*)\}\}/g, "\\operatorname$1{$2}")
    .trimStart()
    .replace(/(?<!\\)\s+$/, "");
  return inline && /\\\s$/.test(out) ? `${out}{}` : out;
}

/** `body` split at its own top-level `\\` (row gaps kept), or null when its
 *  braces or environments don't balance. */
function splitRows(body: string): { rows: string[]; seps: string[] } | null {
  const rows: string[] = [];
  const seps: string[] = [];
  let depth = 0;
  let env = 0;
  let cur = 0;
  for (let i = 0; i < body.length; i++) {
    const c = body[i];
    if (c === "{") depth++;
    else if (c === "}" && --depth < 0) return null;
    else if (c === "\\") {
      if (body.startsWith("\\begin{", i)) env++;
      else if (body.startsWith("\\end{", i) && --env < 0) return null;
      else if (body[i + 1] === "\\" && depth === 0 && env === 0) {
        const gap = /^\\\\\s*(?:\[[^\]]*\])?/.exec(body.slice(i))![0];
        rows.push(body.slice(cur, i).trim());
        seps.push(gap.replace(/\s+/g, ""));
        i += gap.length - 1;
        cur = i + 1;
        continue;
      }
      i++;
    }
  }
  if (depth !== 0 || env !== 0) return null;
  rows.push(body.slice(cur).trim());
  return { rows, seps };
}

/** Rows of a whole-value environment, or null when it isn't one. */
function rowsOf(latex: string): { open: string; rows: string[]; seps: string[]; close: string } | null {
  const m = /^(\\begin\{([a-zA-Z]+\*?)\})([\s\S]*)(\\end\{\2\})$/.exec(latex);
  if (!m) return null;
  let open = m[1];
  let body = m[3];
  // Column spec of the environments that take one.
  const spec = /^(?:array|alignedat|subarray)$/.test(m[2]) ? /^\{[^{}]*\}/.exec(body) : null;
  if (spec) {
    open += spec[0];
    body = body.slice(spec[0].length);
  }
  const split = splitRows(body);
  return split && { open, ...split, close: m[4] };
}

/** A display block one row per line, so Raw mode and diffs read it: an
 *  environment's rows, or the block's own top-level `\\` lines. */
export function layoutBlock(latex: string): string {
  const join = (rows: string[], seps: string[]) =>
    rows.map((row, i) => (i < seps.length ? `${row} ${seps[i]}` : row)).join("\n");
  const env = rowsOf(latex);
  if (env) return env.rows.length < 2 ? latex : `${env.open}\n${join(env.rows, env.seps)}\n${env.close}`;
  const top = splitRows(latex);
  return top && top.rows.length > 1 ? join(top.rows, top.seps) : latex;
}

export const WHOLE_ENV = /^\\begin\{([a-zA-Z]+\*?)\}[\s\S]*\\end\{\1\}$/;

/** Display LaTeX as the field holds it: top-level `\\` lines, which KaTeX
 *  draws but MathLive rejects bare, go inside MathLive's `\displaylines`.
 *  `fromField` takes the wrapper off again, so the note keeps bare lines. */
export function toField(source: string, display: boolean): string {
  if (!display || WHOLE_ENV.test(source)) return source;
  const top = splitRows(source);
  return top && top.rows.length > 1 ? `\\displaylines{${source}}` : source;
}

export function fromField(latex: string): string {
  return latex.startsWith("\\displaylines{") && latex.endsWith("}") ? latex.slice(14, -1).trim() : latex;
}

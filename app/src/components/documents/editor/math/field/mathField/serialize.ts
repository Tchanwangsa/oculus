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

import { findFilesByTail } from "@/lib/db";
import { parsedMdSource } from "@/lib/files/fileTypes";
import type { Citation, CitationShape } from "./parse";

const resolved = new Map<string, Citation | null>();
const resolving = new Map<string, Promise<Citation | null>>();

function shapeKey(s: CitationShape): string {
  return s.kind === "path" ? s.path : `tail:${s.tail}`;
}

/** A tail's file, already looked up: the citation, null for no unique hit,
 *  undefined while unknown. */
export function resolvedNow(shape: CitationShape): Citation | null | undefined {
  if (shape.kind === "path") return { path: shape.path, line: shape.line, page: shape.page };
  const hit = resolved.get(shapeKey(shape));
  return hit === undefined ? undefined : hit && { ...hit, line: shape.line, page: shape.page };
}

/** The library path a tail names, accepting only a unique hit. A parsed
 *  `.md` has no row, so its source is looked up instead — but the citation
 *  keeps the `.md` path, which its line numbers refer to. */
async function lookupTail(tail: string): Promise<string | null> {
  const direct = await findFilesByTail(tail);
  if (direct.length === 1) return direct[0].relative_path;
  if (direct.length > 1) return null;
  const source = parsedMdSource(tail);
  if (!source) return null;
  const rows = await findFilesByTail(source);
  if (rows.length !== 1) return null;
  return rows[0].relative_path.slice(0, -source.length) + tail;
}

export function resolveCitation(shape: CitationShape): Promise<Citation | null> {
  const now = resolvedNow(shape);
  if (now !== undefined) return Promise.resolve(now);
  if (shape.kind === "path") return Promise.resolve(null);
  const key = shapeKey(shape);
  let p = resolving.get(key);
  if (!p) {
    p = lookupTail(shape.tail)
      .then((path) => (path ? { path } : null))
      .catch((e) => {
        console.error(e);
        return null;
      })
      .then((c) => {
        resolved.set(key, c);
        return c;
      });
    resolving.set(key, p);
  }
  return p.then((c) => c && { ...c, line: shape.line, page: shape.page });
}

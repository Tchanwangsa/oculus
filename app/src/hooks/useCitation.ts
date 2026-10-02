import { useEffect, useMemo, useState } from "react";
import {
  citedPage,
  parsedSourceOf,
  resolveCitation,
  resolvedNow,
  type Citation,
  type CitationShape,
} from "@/lib/citations";

/** The file a citation names: at once for a full path, after a cached lookup
 *  for a tail (null until then, and for a tail with no unique hit). Keyed on
 *  the shape's value, so callers may parse it afresh each render. */
export function useCitation(shape: CitationShape | null): Citation | null {
  const key = shape ? JSON.stringify(shape) : "";
  // eslint-disable-next-line react-hooks/exhaustive-deps
  const now = useMemo(() => (shape ? resolvedNow(shape) : null), [key]);
  const [late, setLate] = useState<{ key: string; cite: Citation | null } | null>(null);
  useEffect(() => {
    if (!shape || now !== undefined) return;
    let live = true;
    resolveCitation(shape).then((cite) => live && setLate({ key, cite }));
    return () => {
      live = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, now]);
  if (now !== undefined) return now;
  return late?.key === key ? late.cite : null;
}

/** The PDF page a line of a parsed `.md` falls on, once `.pages.json` is
 *  read (cached); null for any other citation. */
export function useCitedPage(cite: Citation | null | undefined): number | null {
  const path = cite?.line && parsedSourceOf(cite.path) ? cite.path : null;
  const from = cite?.line?.from ?? 0;
  const to = cite?.line?.to ?? 0;
  const [page, setPage] = useState<number | null>(null);
  useEffect(() => {
    setPage(null);
    if (!path) return;
    let live = true;
    citedPage(path, { from, to }).then((hit) => live && setPage(hit?.page ?? null));
    return () => {
      live = false;
    };
  }, [path, from, to]);
  return page;
}

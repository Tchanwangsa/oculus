import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { pageText, type PdfLine } from "@/lib/pdf/pdfView";
import { foldedPattern, matchPage, pageCorpus, partRect, type PdfMatch, type Rect } from "@/lib/pdf/pdfFind";
import type { Highlight } from "./PdfPage";

/** Text fetches in flight while find reads the whole document. */
const FETCH_SLOTS = 4;
/** Matches past this are not counted or painted. */
const MAX_MATCHES = 10_000;

const NONE: ReadonlyMap<number, Highlight[]> = new Map();

/**
 * Find in the PDF: opening the bar reads every page's text once (`pdf_text`,
 * cached per document), a query matches over each page's lines
 * (`lib/pdf/pdfFind.ts`), and every match is painted, the current one stronger.
 * A new query starts at the first match from `startPage()`; `reveal` scrolls
 * the current match into view.
 */
export function usePdfFind(
  path: string,
  pageCount: number,
  startPage: () => number,
  reveal: (page: number, rect: Rect) => void,
) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [texts, setTexts] = useState<PdfLine[][] | null>(null);
  const [current, setCurrent] = useState(-1);
  const loading = useRef(false);

  useEffect(() => {
    if (!open || texts || loading.current || !pageCount) return;
    loading.current = true;
    const out: PdfLine[][] = new Array(pageCount);
    let next = 0;
    let live = true;
    const worker = async () => {
      while (live && next < pageCount) {
        const i = next++;
        out[i] = await pageText(path, i + 1).catch(() => []);
      }
    };
    Promise.all(Array.from({ length: Math.min(FETCH_SLOTS, pageCount) }, worker)).then(() => {
      if (live) setTexts(out);
    });
    return () => {
      live = false;
      loading.current = false;
    };
  }, [open, texts, path, pageCount]);

  const corpora = useMemo(() => texts?.map(pageCorpus) ?? null, [texts]);

  /** Null while the text is still loading. */
  const matches = useMemo<PdfMatch[] | null>(() => {
    if (!corpora) return null;
    const pattern = foldedPattern(query);
    if (!pattern) return [];
    const out: PdfMatch[] = [];
    corpora.forEach((c, i) => {
      if (out.length < MAX_MATCHES) out.push(...matchPage(i + 1, c, pattern, MAX_MATCHES - out.length));
    });
    return out;
  }, [corpora, query]);

  const rectOf = useCallback(
    (m: PdfMatch): Rect => {
      const part = m.parts[0];
      return partRect(texts![m.page - 1][part.line], part.start, part.end);
    },
    [texts],
  );

  const startRef = useRef(startPage);
  startRef.current = startPage;
  const revealRef = useRef(reveal);
  revealRef.current = reveal;

  const select = useCallback(
    (i: number) => {
      setCurrent(i);
      const m = matches?.[i];
      if (m) revealRef.current(m.page, rectOf(m));
    },
    [matches, rectOf],
  );

  // A new query (or the text arriving) starts at the reader's page.
  useEffect(() => {
    if (!matches) return;
    if (!matches.length) return setCurrent(-1);
    const from = startRef.current();
    const i = matches.findIndex((m) => m.page >= from);
    select(i < 0 ? 0 : i);
    // `select` changes with `matches`; this runs once per result set.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [matches]);

  const step = useCallback(
    (backwards: boolean) => {
      if (!matches?.length) return;
      const n = matches.length;
      select(current < 0 ? 0 : (current + (backwards ? n - 1 : 1)) % n);
    },
    [matches, current, select],
  );

  // While the text loads the last status stands, rather than blinking out.
  const lastStatus = useRef<string | undefined>(undefined);
  let status: string | undefined;
  if (!open || !query.trim()) status = undefined;
  else if (!matches) status = lastStatus.current;
  else if (!matches.length) status = "No results";
  else status = current >= 0 ? `${current + 1} of ${matches.length}` : `${matches.length} matches`;
  lastStatus.current = status;

  const highlights = useMemo(() => {
    if (!open || !matches?.length || !texts) return NONE;
    const byPage = new Map<number, Highlight[]>();
    matches.forEach((m, i) => {
      let list = byPage.get(m.page);
      if (!list) byPage.set(m.page, (list = []));
      for (const part of m.parts) {
        const rect = partRect(texts[m.page - 1][part.line], part.start, part.end);
        list.push({ ...rect, current: i === current });
      }
    });
    return byPage;
  }, [open, matches, current, texts]);

  return { open, setOpen, query, setQuery, status, step, highlights };
}

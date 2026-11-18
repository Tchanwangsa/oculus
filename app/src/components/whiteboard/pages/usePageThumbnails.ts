import { useEffect, useRef, useState } from 'react';
import type { CanvasDocument } from '@/lib/canvas/model';
import { pageRects } from '@/lib/canvas/paper';
import { renderPagePng } from '@/lib/canvas/render';
import { pageFingerprints } from '@/lib/canvas/pageFingerprints';

/** CSS pixels across a thumbnail's widest page. */
export const THUMB_WIDTH = 148;
/** How long the content must rest before stale thumbnails are redrawn, so a stroke never waits on them. */
const THUMB_DELAY_MS = 350;

type Cached = { key: string; url: string };

/**
 * The pages whose picture is missing or out of date (`keys` from `pageFingerprints`), nearest the page in view first,
 * so the thumbnails the reader is looking at come back soonest.
 */
export function staleThumbnails(keys: ReadonlyMap<string, string>, cached: ReadonlyMap<string, { key: string }>, pageIds: readonly string[], current: number): string[] {
  return pageIds
    .map((id, index) => ({ id, distance: Math.abs(index - current) }))
    .filter(({ id }) => cached.get(id)?.key !== keys.get(id))
    .sort((a, b) => a.distance - b.distance)
    .map(({ id }) => id);
}

/**
 * Object URLs of small pictures of a notebook's pages, by page id, while `enabled`. A page is redrawn only when its
 * fingerprint changes (its size, template, number or anything touching it), one page at a time once the document has
 * rested, so drawing never waits on them. A pan or zoom keeps the same elements and paper and redraws nothing.
 */
export function usePageThumbnails(document: CanvasDocument, enabled: boolean, current: number): ReadonlyMap<string, string> {
  const [urls, setUrls] = useState<ReadonlyMap<string, string>>(new Map());
  const cache = useRef(new Map<string, Cached>());
  const latest = useRef({ document, current });
  latest.current = { document, current };
  const { elements, paper } = document;

  useEffect(() => {
    if (!enabled || !paper) return;
    let cancelled = false;
    const run = async () => {
      const { document: snapshot, current: inView } = latest.current;
      if (!snapshot.paper) return;
      const keys = pageFingerprints(snapshot), rects = pageRects(snapshot.paper);
      for (const [id, entry] of cache.current) if (!keys.has(id)) { URL.revokeObjectURL(entry.url); cache.current.delete(id); }
      const ratio = Math.min(2, window.devicePixelRatio || 1);
      for (const id of staleThumbnails(keys, cache.current, rects.map(rect => rect.id), inView)) {
        const rect = rects.find(page => page.id === id)!;
        const blob = await renderPagePng(snapshot, id, THUMB_WIDTH * ratio / rect.width).catch(() => null);
        if (cancelled) return;
        if (blob) {
          const previous = cache.current.get(id);
          if (previous) URL.revokeObjectURL(previous.url);
          cache.current.set(id, { key: keys.get(id)!, url: URL.createObjectURL(blob) });
          setUrls(new Map([...cache.current].map(([page, entry]) => [page, entry.url])));
        }
        // One page per task, so input that arrives in between is handled first.
        await new Promise(resolve => setTimeout(resolve, 0));
        if (cancelled) return;
      }
    };
    const timer = setTimeout(() => void run(), THUMB_DELAY_MS);
    return () => { cancelled = true; clearTimeout(timer); };
  }, [elements, paper, enabled]);

  useEffect(() => () => {
    for (const entry of cache.current.values()) URL.revokeObjectURL(entry.url);
    cache.current.clear();
  }, []);

  return urls;
}

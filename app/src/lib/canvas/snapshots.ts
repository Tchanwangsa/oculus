import { invoke } from '@tauri-apps/api/core';
import { blobToBase64 } from '@/lib/encoding/base64';
import { boundsIntersect, elementBounds } from './geometry';
import type { CanvasDocument, CanvasElement } from './model';
import { pageRects } from './paper';
import { renderPagePng } from './render';

/** Pixels per point for the page pictures chat agents read: A4 comes out near 1190 x 1680, enough to read handwriting. */
export const SNAPSHOT_SCALE = 2;
/** How long writing must pause before the pages are redrawn for the agent. */
export const SNAPSHOT_DELAY_MS = 1500;

export interface SnapshotRequest { fileId: number; relativePath: string; pageCount: number; pages: { id: string; index: number; png: string }[] }
export interface SnapshotDeps {
  render: (document: CanvasDocument, pageId: string) => Promise<Blob | null>;
  encode: (blob: Blob) => Promise<string>;
  send: (request: SnapshotRequest) => Promise<void>;
}
export const nativeSnapshotDeps: SnapshotDeps = {
  render: (document, pageId) => renderPagePng(document, pageId, SNAPSHOT_SCALE),
  encode: blobToBase64,
  send: ({ fileId, relativePath, pageCount, pages }) => invoke('canvas_write_page_snapshots', { fileId, relativePath, pageCount, pages }),
};

/** An element's text, kept per element object so an untouched one is never serialised twice. An embedded picture counts by its size and tail. */
const elementKeys = new WeakMap<CanvasElement, string>();
function elementKey(element: CanvasElement): string {
  let key = elementKeys.get(element);
  if (key === undefined) {
    const { dataUrl, ...rest } = element;
    key = JSON.stringify(rest) + (dataUrl ? `|${dataUrl.length}:${dataUrl.slice(-48)}` : '');
    elementKeys.set(element, key);
  }
  return key;
}

/** What each page's picture depends on, by page id: its number, size and template, and every element touching it. */
export function pageFingerprints(document: CanvasDocument): Map<string, string> {
  const { paper } = document;
  if (!paper) return new Map();
  return new Map(pageRects(paper).map(rect => {
    const touching = document.elements.filter(element => boundsIntersect(elementBounds(element), rect));
    return [rect.id, `${rect.index} ${rect.width}x${rect.height} ${rect.template}\n${touching.map(elementKey).join('\n')}`];
  }));
}

const message = (error: unknown) => error instanceof Error ? error.message : String(error);

/**
 * Keeps one notebook's page pictures current: a moment after the content stops changing it redraws only the pages
 * that differ from what was last written and sends them. Writes run one at a time, a failed one is retried by the
 * next change, and a pan or zoom (which changes no page) writes nothing.
 */
export class NotebookSnapshots {
  private written = new Map<string, string>();
  private path = '';
  private count = 0;
  private timer: ReturnType<typeof setTimeout> | undefined;
  private latest: { document: CanvasDocument; path: string } | undefined;
  private queue: Promise<void> = Promise.resolve();

  constructor(
    private readonly fileId: number,
    private readonly deps: SnapshotDeps = nativeSnapshotDeps,
    private readonly onError: (error: string | null) => void = () => {},
    private readonly delay = SNAPSHOT_DELAY_MS,
  ) {}

  schedule(document: CanvasDocument, path: string): void {
    if (!document.paper) return;
    this.latest = { document, path };
    clearTimeout(this.timer);
    this.timer = setTimeout(() => void this.flush(), this.delay);
  }

  /** Writes now what `schedule` last saw; resolves when that write (and any before it) has finished. */
  flush(): Promise<void> {
    clearTimeout(this.timer);
    this.timer = undefined;
    this.queue = this.queue.then(() => this.drain());
    return this.queue;
  }

  private async drain(): Promise<void> {
    const job = this.latest;
    this.latest = undefined;
    const paper = job?.document.paper;
    if (!job || !paper) return;
    try {
      const rects = pageRects(paper), keys = pageFingerprints(job.document);
      const changed = rects.filter(rect => this.written.get(rect.id) !== keys.get(rect.id));
      if (!changed.length && job.path === this.path && rects.length === this.count) { this.onError(null); return; }
      const pages: SnapshotRequest['pages'] = [];
      for (const rect of changed) {
        const blob = await this.deps.render(job.document, rect.id);
        if (blob) pages.push({ id: rect.id, index: rect.index, png: await this.deps.encode(blob) });
      }
      await this.deps.send({ fileId: this.fileId, relativePath: job.path, pageCount: rects.length, pages });
      this.written = keys; this.path = job.path; this.count = rects.length;
      this.onError(null);
    } catch (error) {
      // Nothing is marked written, so the next change tries these pages again.
      this.latest ??= job;
      this.onError(message(error));
    }
  }

  dispose(): void {
    clearTimeout(this.timer);
    this.timer = undefined;
  }
}

/** Drops a deleted notebook's page pictures. */
export function forgetPageSnapshots(fileId: number): void {
  invoke('canvas_delete_page_snapshots', { fileId }).catch(error => console.error('[canvas] page pictures were not removed', error));
}

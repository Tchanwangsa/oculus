import { useEffect, useRef, useState } from 'react';
import { XIcon } from '@phosphor-icons/react';
import { Button } from '@/components/ui/button';
import { ConfirmDialog } from '@/components/ui/ConfirmDialog';
import { useScrollFade } from '@/hooks/ui/useScrollFade';
import type { useWhiteboardInteraction } from '@/hooks/whiteboard/useWhiteboardInteraction';
import type { WhiteboardDocument } from '@/lib/whiteboard/model';
import { useWhiteboardPlatform } from '@/lib/whiteboard/platform';
import { WhiteboardImageDialog } from '../WhiteboardImageDialog';
import { AddPageMenu } from './AddPageMenu';
import { PageThumbnail } from './PageThumbnail';
import { usePageReorder } from './usePageReorder';
import { usePageThumbnails } from './usePageThumbnails';

export type NotebookPages = NonNullable<ReturnType<typeof useWhiteboardInteraction>['pages']>;

/**
 * A notebook's pages as a column of thumbnails beside the drawing: the page in view is marked, a click goes to a page,
 * a drag reorders, each page's ⋯ menu edits it, and + adds a blank page or PDF pages at the end. The list is the column's one scroller.
 * Below the whiteboard's narrow width it lies over the drawing instead of beside it (`whiteboard.css`).
 */
export function PageSidebar({ pages, document, onClose }: { pages: NotebookPages; document: WhiteboardDocument; onClose: () => void }) {
  const platform = useWhiteboardPlatform();
  const scroller = useRef<HTMLDivElement>(null);
  const [removing, setRemoving] = useState<number | null>(null);
  const [pdfAfter, setPdfAfter] = useState<number | null>(null);
  const urls = usePageThumbnails(document, true, pages.index);
  const reorder = usePageReorder(scroller, pages.count, pages.move);
  useScrollFade(scroller, 'y', pages.count);
  const widest = Math.max(...pages.paper.pages.map(page => page.width));

  useEffect(() => {
    scroller.current?.querySelector('[aria-current="page"]')?.scrollIntoView({ block: 'nearest' });
  }, [pages.index]);

  return <aside className="whiteboard-page-sidebar flex h-full w-[188px] shrink-0 flex-col border-r border-border-subtle bg-card" aria-label="Pages">
    <div className="flex h-11 shrink-0 items-center gap-2 pr-1.5 pl-3.5">
      <span className="text-[12px] font-medium text-foreground">Pages</span>
      <span className="text-[11px] tabular-nums text-muted-foreground">{pages.count}</span>
      <Button variant="ghost" size="icon-sm" className="ml-auto rounded-lg text-muted-foreground" aria-label="Close pages" onClick={onClose}><XIcon size={15} /></Button>
    </div>
    <div ref={scroller} className="relative min-h-0 flex-1 overflow-y-auto px-2 pb-3">
      {pages.paper.pages.map((page, index) => <PageThumbnail key={page.id} page={page} index={index} widest={widest} current={index === pages.index} url={urls.get(page.id)}
        dragging={reorder.drag?.from === index} held={reorder.held === index} canRemove={pages.count > 1} itemRef={reorder.itemRef(index)}
        onGo={() => { if (!reorder.didDrag()) pages.go(index); }} onPointerDown={event => reorder.onPointerDown(event, index)}
        actions={{
          insert: side => pages.insert(index, side),
          duplicate: () => pages.duplicate(index),
          setTemplate: template => pages.setTemplate(index, template),
          applyTemplateToAll: () => pages.applyTemplateToAll(index),
          setSize: size => pages.setSize(index, size),
          remove: () => setRemoving(index),
        }} />)}
      {reorder.drag ? <div aria-hidden="true" className="pointer-events-none absolute right-3 left-3 h-0.5 -translate-y-1/2 rounded-full bg-brand" style={{ top: reorder.drag.top }} /> : null}
      <AddPageMenu onAdd={pages.add} onPdf={platform.libraryPdfs ? () => setPdfAfter(pages.count - 1) : undefined} />
    </div>
    <ConfirmDialog open={removing !== null} onCancel={() => setRemoving(null)} title={`Delete page ${(removing ?? 0) + 1}?`}
      description="The page and everything drawn on it are removed, and the pages after it move up. Undo brings it back."
      confirmLabel="Delete page" onConfirm={() => { const index = removing; setRemoving(null); if (index !== null) pages.remove(index); }} />
    {platform.libraryPdfs ? <WhiteboardImageDialog kind="notebook" open={pdfAfter !== null} onOpenChange={open => { if (!open) setPdfAfter(null); }}
      pdfPages={{ after: (pdfAfter ?? 0) + 1, onInsert: (sizes, images) => { if (pdfAfter !== null) pages.insertPdfPages(pdfAfter, sizes, images); } }} /> : null}
  </aside>;
}

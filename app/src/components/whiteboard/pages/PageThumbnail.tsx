import type { PointerEvent, Ref } from 'react';
import type { PaperPage } from '@/lib/canvas/paper';
import { DRAG_SURFACE } from '@/hooks/gestures/usePointerDrag';
import { cn } from '@/lib/utils';
import { PageMenu, type PageMenuActions } from './PageMenu';
import { THUMB_WIDTH } from './usePageThumbnails';

/**
 * One page in the sidebar: its picture at the page's proportions (blank paper until the picture is drawn), its number,
 * and its ⋯ menu. The picture is the button that goes to the page and the handle that drags it; the menu is a sibling,
 * never nested in it.
 */
export function PageThumbnail({ page, index, widest, current, url, dragging, held, canRemove, actions, itemRef, onGo, onPointerDown }: {
  page: PaperPage;
  index: number;
  /** The widest page's width, which fills `THUMB_WIDTH`; narrower pages draw to the same scale. */
  widest: number;
  current: boolean;
  url?: string;
  dragging: boolean;
  held: boolean;
  canRemove: boolean;
  actions: PageMenuActions;
  itemRef: Ref<HTMLDivElement>;
  onGo: () => void;
  onPointerDown: (event: PointerEvent<HTMLElement>) => void;
}) {
  const number = index + 1, scale = THUMB_WIDTH / widest;
  return <div ref={itemRef} className={cn('relative flex flex-col items-center gap-1.5 py-2', dragging && 'opacity-40')}>
    <div className="relative">
      <button type="button" aria-label={`Page ${number}`} aria-current={current ? 'page' : undefined} onClick={onGo} onPointerDown={onPointerDown}
        className={cn(DRAG_SURFACE, 'block cursor-pointer rounded-[3px] bg-white shadow-xs outline-none [-webkit-touch-callout:none] focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-card',
          current ? 'ring-2 ring-brand ring-offset-2 ring-offset-card' : 'ring-1 ring-border', held && 'scale-[1.03] shadow-md')}
        style={{ width: page.width * scale, height: page.height * scale }}>
        {url ? <img src={url} alt="" draggable={false} className="pointer-events-none size-full rounded-[3px]" /> : null}
      </button>
      <div className="absolute top-1.5 right-1.5"><PageMenu page={page} number={number} canRemove={canRemove} actions={actions} /></div>
    </div>
    <span className={cn('text-[11px] tabular-nums', current ? 'font-medium text-brand' : 'text-muted-foreground')}>{number}</span>
  </div>;
}

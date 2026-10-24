import { useState } from 'react';
import { CaretLeftIcon, CaretRightIcon, PlusIcon, TrashIcon } from '@phosphor-icons/react';
import { ConfirmDialog } from '@/components/ui/ConfirmDialog';
import { CanvasIconButton } from './CanvasToolbar';

export interface CanvasPageControls { count: number; index: number; go: (index: number) => void; add: () => void; remove: (index: number) => void }

/** A notebook's page controls: which page is in view, steps between pages, a new last page, and removing the one in view. */
export function CanvasPageBar({ pages }: { pages: CanvasPageControls }) {
  const [confirming, setConfirming] = useState(false);
  const number = pages.index + 1;
  return <>
    <div className="flex items-center rounded-xl border border-border-subtle bg-card p-1 shadow-xs" role="group" aria-label="Notebook pages">
      <CanvasIconButton label="Previous page" className="size-10" onClick={() => pages.go(pages.index - 1)} disabled={pages.index === 0}><CaretLeftIcon size={15} /></CanvasIconButton>
      <span role="status" className="min-w-20 px-1 text-center text-[12px] tabular-nums text-muted-foreground">Page {number} of {pages.count}</span>
      <CanvasIconButton label="Next page" className="size-10" onClick={() => pages.go(pages.index + 1)} disabled={pages.index >= pages.count - 1}><CaretRightIcon size={15} /></CanvasIconButton>
      <div className="ml-1 flex border-l border-border-subtle pl-1">
        <CanvasIconButton label="Add page" className="size-10" onClick={pages.add}><PlusIcon size={16} /></CanvasIconButton>
        <CanvasIconButton label={`Remove page ${number}`} className="size-10" onClick={() => setConfirming(true)} disabled={pages.count < 2}><TrashIcon size={16} /></CanvasIconButton>
      </div>
    </div>
    <ConfirmDialog open={confirming} onCancel={() => setConfirming(false)} title={`Remove page ${number}?`}
      description="The page and everything drawn on it are removed, and the pages after it move up. Undo brings it back."
      confirmLabel="Remove page" onConfirm={() => { setConfirming(false); pages.remove(pages.index); }} />
  </>;
}

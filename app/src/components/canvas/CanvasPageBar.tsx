import { useState } from 'react';
import { CaretLeftIcon, CaretRightIcon, NotebookIcon, PlusIcon, TrashIcon } from '@phosphor-icons/react';
import { Button } from '@/components/ui/button';
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover';
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip';
import { PAPER_TEMPLATES, type PaperTemplate } from '@/lib/canvas/paper';
import { cn } from '@/lib/utils';
import { Segmented } from './inspector/Segmented';
import { PaperThumb, TEMPLATE_LABELS } from './PaperThumb';
import { ConfirmDialog } from '@/components/ui/ConfirmDialog';
import { CanvasIconButton } from './CanvasToolbar';

export interface CanvasPageControls {
  count: number; index: number; go: (index: number) => void; add: () => void; remove: (index: number) => void;
  /** The template of the page in view, and a change of it for that page or (`every`) for all of them. */
  template: PaperTemplate; setTemplate: (template: PaperTemplate, every: boolean) => void;
}

/** The paper of the page in view: pick a template for this page or for every page. Each pick is one undo step. */
function PageTemplateMenu({ pages }: { pages: CanvasPageControls }) {
  const [scope, setScope] = useState<'page' | 'all'>('page');
  return <Popover>
    <Tooltip>
      <TooltipTrigger asChild><PopoverTrigger asChild><Button type="button" variant="ghost" size="icon" aria-label="Paper template" className="size-10 rounded-lg text-muted-foreground"><NotebookIcon size={16} /></Button></PopoverTrigger></TooltipTrigger>
      <TooltipContent side="top" sideOffset={8}>Paper template</TooltipContent>
    </Tooltip>
    <PopoverContent side="top" align="end" className="w-[272px] p-3">
      <div className="mb-2.5 flex items-center justify-between gap-3">
        <span className="text-[13px] font-medium">Paper</span>
        <Segmented label="Apply to" value={scope} onChange={setScope} className="w-36"
          options={[{ value: 'page', label: `Page ${pages.index + 1} only`, content: <span className="text-[11px]">This page</span> }, { value: 'all', label: 'Every page', content: <span className="text-[11px]">All pages</span> }]} />
      </div>
      <div role="group" aria-label="Paper template" className="grid grid-cols-4 gap-1">
        {PAPER_TEMPLATES.map(value => {
          const active = scope === 'page' && value === pages.template;
          return <button key={value} type="button" aria-pressed={active} onClick={() => pages.setTemplate(value, scope === 'all')}
            className={cn('flex cursor-pointer flex-col items-center gap-1 rounded-lg border border-transparent p-1.5 text-[11px] text-muted-foreground outline-none transition-colors hover:bg-accent hover:text-foreground focus-visible:ring-2 focus-visible:ring-brand',
              active && 'border-brand bg-brand/12 text-brand hover:bg-brand/18 hover:text-brand')}>
            <PaperThumb template={value} scale={0.7} />{TEMPLATE_LABELS[value]}
          </button>;
        })}
      </div>
    </PopoverContent>
  </Popover>;
}

/** A notebook's page controls: which page is in view, steps between pages, its paper, a new last page, and removing the one in view. */
export function CanvasPageBar({ pages }: { pages: CanvasPageControls }) {
  const [confirming, setConfirming] = useState(false);
  const number = pages.index + 1;
  return <>
    <div className="flex items-center rounded-xl border border-border-subtle bg-card p-1 shadow-xs" role="group" aria-label="Notebook pages">
      <CanvasIconButton label="Previous page" className="size-10" onClick={() => pages.go(pages.index - 1)} disabled={pages.index === 0}><CaretLeftIcon size={15} /></CanvasIconButton>
      <span role="status" className="min-w-20 px-1 text-center text-[12px] tabular-nums text-muted-foreground">Page {number} of {pages.count}</span>
      <CanvasIconButton label="Next page" className="size-10" onClick={() => pages.go(pages.index + 1)} disabled={pages.index >= pages.count - 1}><CaretRightIcon size={15} /></CanvasIconButton>
      <div className="ml-1 flex border-l border-border-subtle pl-1">
        <PageTemplateMenu pages={pages} />
        <CanvasIconButton label="Add page" className="size-10" onClick={pages.add}><PlusIcon size={16} /></CanvasIconButton>
        <CanvasIconButton label={`Remove page ${number}`} className="size-10" onClick={() => setConfirming(true)} disabled={pages.count < 2}><TrashIcon size={16} /></CanvasIconButton>
      </div>
    </div>
    <ConfirmDialog open={confirming} onCancel={() => setConfirming(false)} title={`Remove page ${number}?`}
      description="The page and everything drawn on it are removed, and the pages after it move up. Undo brings it back."
      confirmLabel="Remove page" onConfirm={() => { setConfirming(false); pages.remove(pages.index); }} />
  </>;
}

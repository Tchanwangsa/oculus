import { useState } from 'react';
import type { CanvasArrowhead } from '@/lib/canvas/model';
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover';
import { cn } from '@/lib/utils';
import { ArrowheadGlyph } from './ArrowheadGlyph';
import { inspectorPopoverProps } from './popoverGuards';
import { Segmented } from './Segmented';

const HEADS: [CanvasArrowhead, string][] = [['none', 'None'], ['arrow', 'Arrow'], ['triangle', 'Triangle'], ['dot', 'Dot'], ['bar', 'Bar'], ['diamond', 'Diamond']];

/** One end's head as a button showing it; the six types open in a grid beside the panel. */
export function ArrowheadPicker({ end, value, onChange }: { end: 'start' | 'end'; value: CanvasArrowhead; onChange: (head: CanvasArrowhead) => void }) {
  const [open, setOpen] = useState(false);
  const name = end === 'start' ? 'Start' : 'End';
  const current = HEADS.find(([head]) => head === value)?.[1] ?? value;
  return <Popover open={open} onOpenChange={setOpen}>
    <PopoverTrigger asChild>
      <button type="button" aria-label={`${name} arrowhead: ${current}`}
        className={cn('flex h-8 flex-1 cursor-pointer items-center justify-center gap-1.5 rounded-[10px] bg-muted text-foreground transition-[background-color,box-shadow] outline-none hover:bg-accent focus-visible:ring-2 focus-visible:ring-brand',
          open && 'bg-card text-brand shadow-xs ring-1 ring-border-subtle')}>
        <span className="text-[11px] text-muted-foreground">{name}</span>
        <ArrowheadGlyph type={value} end={end} />
      </button>
    </PopoverTrigger>
    <PopoverContent side="right" align="start" collisionPadding={12} className="w-auto p-1.5" {...inspectorPopoverProps}>
      <Segmented label={`${name} arrowhead`} className="grid w-[8.75rem] grid-cols-3" value={value}
        onChange={head => { onChange(head); setOpen(false); }}
        options={HEADS.map(([head, label]) => ({ value: head, label, content: <ArrowheadGlyph type={head} end={end} /> }))} />
    </PopoverContent>
  </Popover>;
}

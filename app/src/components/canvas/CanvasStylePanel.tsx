import { useRef, type MouseEvent } from 'react';
import { SlidersHorizontalIcon } from '@phosphor-icons/react';
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover';
import { Button } from '@/components/ui/button';
import { useScrollFade } from '@/hooks/ui/useScrollFade';
import { InspectorControls, type InspectorProps } from './inspector/InspectorControls';

/** Keeps focus in an open text overlay so a style change restyles what is being written instead of committing it. */
const keepTextFocus = (event: MouseEvent) => { if (!(event.target as Element).closest('input')) event.preventDefault(); };

/** A card beside the canvas on wide panes, a popover behind a Style button on narrow ones; either scrolls inside rather than overflowing. */
export function CanvasStylePanel(props: InspectorProps) {
  const scroller = useRef<HTMLDivElement>(null);
  useScrollFade(scroller);
  return <Popover>
    <div className="pointer-events-none absolute top-[5.5rem] bottom-[4.5rem] left-4 z-10 flex flex-col items-start">
      <PopoverTrigger asChild><Button variant="outline" size="sm" className="canvas-style-trigger pointer-events-auto gap-2 bg-card shadow-xs"><SlidersHorizontalIcon size={16} />Style</Button></PopoverTrigger>
      <div onMouseDown={keepTextFocus} className="canvas-style-desktop pointer-events-auto flex max-h-full w-[212px] flex-col overflow-hidden rounded-xl border border-border-subtle bg-card shadow-xs">
        <div ref={scroller} className="min-h-0 overflow-y-auto overscroll-contain p-3.5"><InspectorControls {...props} /></div>
      </div>
    </div>
    <PopoverContent onMouseDown={keepTextFocus} side="bottom" align="start" className="max-h-[var(--radix-popover-content-available-height)] w-[212px] overflow-y-auto p-3.5">
      <InspectorControls {...props} />
    </PopoverContent>
  </Popover>;
}

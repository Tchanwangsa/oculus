import { useState } from 'react';
import { CaretDownIcon } from '@phosphor-icons/react';
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover';
import { cn } from '@/lib/utils';
import type { WhiteboardElement, WhiteboardPen } from '@/lib/whiteboard/model';
import { MARKERS, PEN_LABELS, PENS, isMarker, nibOutline, penLine } from '@/lib/whiteboard/pens';
import type { SectionProps } from './InspectorSection';
import { inspectorPopoverProps } from './popoverGuards';

const SAMPLE_POINTS = Array.from({ length: 13 }, (_, i) => ({ x: 4 + i * 4, y: 10 + Math.sin(i / 1.9) * 5 }));
const SAMPLE_PATH = SAMPLE_POINTS.map((p, i) => `${i ? 'L' : 'M'}${p.x} ${p.y.toFixed(1)}`).join('');

/** A short wave drawn with the pen itself, by the geometry the whiteboard uses, in the current text colour. */
function PenSample({ pen }: { pen: WhiteboardPen }) {
  const element = { type: 'freedraw', pen, strokeWidth: isMarker(pen) ? 7 : 2.4, seed: 3, points: SAMPLE_POINTS } as WhiteboardElement;
  const line = penLine(element);
  return <svg viewBox="0 0 56 20" className="h-5 w-14 shrink-0" aria-hidden="true">
    {line ? line.passes.map((pass, i) => <path key={i} d={SAMPLE_PATH} fill="none" stroke="currentColor" strokeWidth={pass.width} strokeLinecap={line.cap} strokeLinejoin={line.join}
      strokeDasharray={line.dash.join(' ') || undefined} opacity={pass.alpha * (isMarker(pen) ? 0.45 : 1)} transform={`translate(${pass.dx} ${pass.dy})`} />)
      : <path d={nibOutline(element)} fill="currentColor" />}
  </svg>;
}

/** The pens a stroke can change to: the highlighter's markers for a marker, the writing pens for any other. */
function PenChoices({ values, onChange, className }: SectionProps & { className?: string }) {
  const pens = isMarker(values.pen) ? MARKERS : PENS;
  return <div role="group" aria-label="Pen" className={cn('grid grid-cols-1 gap-0.5', className)}>
    {pens.map(pen => {
      const active = pen === values.pen;
      return <button key={pen} type="button" aria-pressed={active} onClick={() => onChange({ pen })}
        className={cn('flex h-8 cursor-pointer items-center gap-2 rounded-lg px-1.5 text-left text-xs text-muted-foreground outline-none transition-[color,background-color] hover:bg-accent hover:text-foreground focus-visible:ring-2 focus-visible:ring-brand',
          active && 'bg-brand/12 text-brand hover:bg-brand/18 hover:text-brand')}>
        <PenSample pen={pen} /><span className="truncate">{PEN_LABELS[pen]}</span>
      </button>;
    })}
  </div>;
}

/** The style bar's pen picker: the current pen's sample and name, opening the choices; narrow panes drop the name (`whiteboard.css`). */
export function PenMenu(props: SectionProps) {
  const [open, setOpen] = useState(false);
  return <Popover open={open} onOpenChange={setOpen}>
    <PopoverTrigger asChild>
      <button type="button" aria-label={`Pen: ${PEN_LABELS[props.values.pen]}`} className="flex h-8 shrink-0 cursor-pointer items-center gap-1.5 rounded-lg px-1.5 text-xs text-muted-foreground outline-none hover:bg-accent hover:text-foreground focus-visible:ring-2 focus-visible:ring-brand">
        <PenSample pen={props.values.pen} /><span className="whiteboard-bar-pen-name max-w-28 truncate">{PEN_LABELS[props.values.pen]}</span><CaretDownIcon size={12} />
      </button>
    </PopoverTrigger>
    <PopoverContent {...inspectorPopoverProps} side="bottom" align="start" className="w-52 p-1.5"><PenChoices {...props} onChange={patch => { props.onChange(patch); setOpen(false); }} /></PopoverContent>
  </Popover>;
}

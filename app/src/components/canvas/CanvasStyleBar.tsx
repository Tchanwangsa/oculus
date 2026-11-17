import { type MouseEvent } from 'react';
import { SlidersHorizontalIcon, TrashIcon } from '@phosphor-icons/react';
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover';
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip';
import { Button } from '@/components/ui/button';
import { CanvasIconButton } from './CanvasToolbar';
import { ColorRow } from './inspector/ColorRow';
import { InspectorControls, type InspectorProps } from './inspector/InspectorControls';
import { FILL_PALETTE, FILL_PICKS, MARKER_PICKS, STROKE_PALETTE, STROKE_PICKS } from './inspector/palette';
import { PenMenu } from './inspector/PenSection';
import { isMarker } from '@/lib/canvas/pens';
import { inspectorPopoverProps } from './inspector/popoverGuards';
import { StrokeWidthControl } from './inspector/StrokeWidthSection';

/** Keeps focus in an open text overlay so a style change restyles what is being written instead of committing it. */
const keepTextFocus = (event: MouseEvent) => { if (!(event.target as Element).closest('input')) event.preventDefault(); };
const divider = <span aria-hidden className="h-5 w-px shrink-0 bg-border" />;

/**
 * The style half of the canvas's top bar: colour, pen, fill and stroke width for the selection or the tool's next element,
 * then a Style button that opens every section that applies. Narrow panes drop the inline controls (`canvas.css`);
 * the popover always has them all.
 */
export function CanvasStyleBar(props: InspectorProps) {
  const { sections, values, colorLabel, onChange, onPreview, selectionCount, onDelete } = props;
  const has = (section: InspectorProps['sections'][number]) => sections.includes(section);
  return <div data-canvas-inspector="true" onMouseDown={keepTextFocus} className="flex shrink-0 items-center gap-2.5">
    {has('stroke') ? <div className="canvas-bar-stroke flex items-center gap-2.5">
      <ColorRow label={colorLabel} value={values.stroke} onChange={stroke => onChange({ stroke })} picks={has('pen') && isMarker(values.pen) ? MARKER_PICKS : STROKE_PICKS} palette={STROKE_PALETTE} side="bottom" />
      {divider}
    </div> : null}
    {has('pen') ? <div className="flex items-center gap-2.5">
      <PenMenu values={values} onChange={onChange} />
      {divider}
    </div> : null}
    {has('fill') ? <div className="canvas-bar-fill flex items-center gap-2.5">
      <ColorRow label="Fill" value={values.fill} onChange={fill => onChange({ fill })} picks={FILL_PICKS} palette={FILL_PALETTE} side="bottom" />
      {divider}
    </div> : null}
    {has('strokeWidth') ? <div className="canvas-bar-width flex items-center gap-2.5">
      <StrokeWidthControl values={values} onChange={onChange} onPreview={onPreview} className="w-40" />
      {divider}
    </div> : null}
    <Popover>
      <Tooltip>
        <TooltipTrigger asChild><PopoverTrigger asChild><Button variant="ghost" size="icon" aria-label="Style" className="shrink-0 rounded-lg text-muted-foreground"><SlidersHorizontalIcon size={17} /></Button></PopoverTrigger></TooltipTrigger>
        <TooltipContent side="bottom" sideOffset={8}>Style</TooltipContent>
      </Tooltip>
      <PopoverContent {...inspectorPopoverProps} onMouseDown={keepTextFocus} side="bottom" align="start" className="max-h-[var(--radix-popover-content-available-height)] w-[212px] overflow-y-auto p-3.5">
        <InspectorControls {...props} />
      </PopoverContent>
    </Popover>
    {selectionCount ? <CanvasIconButton label="Delete selection" onClick={onDelete} className="shrink-0 hover:text-destructive"><TrashIcon size={17} /></CanvasIconButton> : null}
  </div>;
}

import { useState } from 'react';
import { SlidersHorizontalIcon } from '@phosphor-icons/react';
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover';
import type { InspectorSection, StylePatch } from '@/lib/canvas/style';
import { CanvasIconButton } from '../CanvasToolbar';
import { ArrowheadSection } from './ArrowheadSection';
import { ArrowTypeSection } from './ArrowTypeSection';
import type { SectionProps } from './InspectorSection';
import { OpacitySection } from './OpacitySection';
import { inspectorPopoverProps, keepTextFocus } from './popoverGuards';
import { RoundnessSection } from './RoundnessSection';
import { SketchSection } from './SketchSection';
import { StrokeStyleSection } from './StrokeStyleSection';
import { TextSection } from './TextSection';

/** The sections the style bar keeps behind More styles rather than inline. */
const MORE: InspectorSection[] = ['text', 'strokeStyle', 'sketch', 'roundness', 'arrowType', 'arrowheads', 'opacity'];
export const hasMoreStyles = (sections: readonly InspectorSection[]) => MORE.some(section => sections.includes(section));

/** One button that opens the rest of what applies below the bar, as a column of sections in the inspector's fixed order. */
export function MoreStyles({ sections, values, onChange, onPreview }: SectionProps & { sections: readonly InspectorSection[]; onPreview: (patch: StylePatch) => void }) {
  const [open, setOpen] = useState(false);
  const has = (section: InspectorSection) => sections.includes(section);
  const props = { values, onChange };
  return <Popover open={open} onOpenChange={setOpen}>
    <PopoverTrigger asChild>
      <CanvasIconButton label="More styles" active={open} className="size-8"><SlidersHorizontalIcon size={17} /></CanvasIconButton>
    </PopoverTrigger>
    <PopoverContent {...inspectorPopoverProps} onMouseDown={keepTextFocus} side="bottom" align="end" sideOffset={8} collisionPadding={12}
      className="canvas-chrome max-h-[var(--radix-popover-content-available-height)] w-[212px] overflow-y-auto p-3.5">
      <div className="flex flex-col gap-3.5 text-xs">
        {has('text') ? <TextSection {...props} vertical={has('verticalAlign')} /> : null}
        {has('strokeStyle') ? <StrokeStyleSection {...props} /> : null}
        {has('sketch') ? <SketchSection {...props} /> : null}
        {has('roundness') ? <RoundnessSection {...props} /> : null}
        {has('arrowType') ? <ArrowTypeSection {...props} /> : null}
        {has('arrowheads') ? <ArrowheadSection {...props} /> : null}
        {has('opacity') ? <OpacitySection {...props} onPreview={onPreview} /> : null}
      </div>
    </PopoverContent>
  </Popover>;
}

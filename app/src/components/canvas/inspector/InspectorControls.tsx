import { TrashIcon } from '@phosphor-icons/react';
import type { InspectorSection, StylePatch, StyleValues } from '@/lib/canvas/style';
import type { LayerMove } from '@/lib/canvas/layers';
import { CanvasIconButton } from '../CanvasToolbar';
import { ArrowheadSection } from './ArrowheadSection';
import { ColorSection } from './ColorSection';
import { FillSection } from './FillSection';
import { LayerSection } from './LayerSection';
import { OpacitySection } from './OpacitySection';
import { SketchSection } from './SketchSection';
import { StrokeStyleSection } from './StrokeStyleSection';
import { StrokeWidthSection } from './StrokeWidthSection';
import { TextSection } from './TextSection';

export interface InspectorProps {
  sections: InspectorSection[];
  values: StyleValues;
  colorLabel: string;
  onChange: (patch: StylePatch) => void;
  onPreview: (patch: StylePatch) => void;
  selectionCount: number;
  onDelete: () => void;
  layerMoves: LayerMove[];
  onReorder: (move: LayerMove) => void;
}

/** Sections come from what is selected (or the tool's next element), in a fixed order; Layer shows with a selection. */
export function InspectorControls({ sections, values, colorLabel, onChange, onPreview, selectionCount, onDelete, layerMoves, onReorder }: InspectorProps) {
  const has = (section: InspectorSection) => sections.includes(section);
  const props = { values, onChange };
  return <div className="flex flex-col gap-3.5 text-xs">
    {has('stroke') ? <ColorSection {...props} label={colorLabel} /> : null}
    {has('fill') ? <FillSection {...props} /> : null}
    {has('text') ? <TextSection {...props} vertical={has('verticalAlign')} /> : null}
    {has('strokeWidth') ? <StrokeWidthSection {...props} /> : null}
    {has('strokeStyle') ? <StrokeStyleSection {...props} /> : null}
    {has('sketch') ? <SketchSection {...props} /> : null}
    {has('arrowheads') ? <ArrowheadSection {...props} /> : null}
    {has('opacity') ? <OpacitySection {...props} onPreview={onPreview} /> : null}
    {selectionCount ? <LayerSection moves={layerMoves} onReorder={onReorder} /> : null}
    {selectionCount ? <div className="flex items-center justify-between border-t border-border-subtle pt-2.5">
      <span className="text-[11px] text-muted-foreground tabular-nums">{selectionCount} selected</span>
      <CanvasIconButton label="Delete selection" onClick={onDelete} className="size-7 hover:text-destructive"><TrashIcon size={15} /></CanvasIconButton>
    </div> : null}
  </div>;
}

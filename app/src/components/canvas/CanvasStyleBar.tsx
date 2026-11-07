import { useRef } from 'react';
import type { LayerMove } from '@/lib/canvas/layers';
import type { InspectorSection, StylePatch, StyleValues } from '@/lib/canvas/style';
import type { CanvasToolPreferences } from '@/lib/canvas/toolPreferences';
import { useScrollFade } from '@/hooks/ui/useScrollFade';
import { ColorRow } from './inspector/ColorRow';
import { EraserControls } from './inspector/EraserControls';
import { FillButton } from './inspector/FillButton';
import { FontSizePicker } from './inspector/FontSizePicker';
import { hasMoreStyles, MoreStyles } from './inspector/MoreStyles';
import { SelectionActions } from './inspector/SelectionActions';
import { StrokeWidthPicker } from './inspector/StrokeWidthPicker';
import { StyleGroup as Group } from './inspector/StyleGroup';
import { STROKE_PALETTE, STROKE_PICKS } from './inspector/palette';
import { keepTextFocus } from './inspector/popoverGuards';

export interface StyleBarProps {
  sections: InspectorSection[];
  values: StyleValues;
  colorLabel: string;
  onChange: (patch: StylePatch) => void;
  onPreview: (patch: StylePatch) => void;
  selectionCount: number;
  onDelete: () => void;
  onCrop?: () => void;
  layerMoves: LayerMove[];
  onReorder: (move: LayerMove) => void;
  /** With the eraser chosen: its device-local settings, shown in place of styles. */
  eraser?: { preferences: CanvasToolPreferences; onChange: (patch: Partial<CanvasToolPreferences>) => void };
}

/**
 * The inspector as a slim bar centred under the tool bar (`canvas.css` places
 * it), holding only what applies: colour, fill, stroke width and font size
 * inline, the rest behind More styles, then the selection's actions; with
 * the eraser, its settings. Its popovers open below it. A pane too narrow
 * for it scrolls it sideways.
 */
export function CanvasStyleBar({ sections, values, colorLabel, onChange, onPreview, selectionCount, onDelete, onCrop, layerMoves, onReorder, eraser }: StyleBarProps) {
  const scroller = useRef<HTMLDivElement>(null);
  useScrollFade(scroller, 'x');
  const has = (section: InspectorSection) => sections.includes(section);
  const props = { values, onChange };
  return <div data-canvas-inspector="true" role="toolbar" aria-label={eraser ? 'Eraser' : 'Style'} onMouseDown={keepTextFocus}
    className="canvas-style-bar canvas-chrome absolute left-1/2 z-10 flex max-w-[calc(100%-2rem)] -translate-x-1/2 overflow-hidden rounded-2xl border border-border-subtle shadow-sm">
    <div ref={scroller} className="canvas-style-scroller min-w-0 overflow-x-auto overscroll-contain p-1.5">
      <div className="flex w-max items-center">
        {has('stroke') ? <Group><ColorRow label={colorLabel} value={values.stroke} onChange={stroke => onChange({ stroke })} picks={STROKE_PICKS} palette={STROKE_PALETTE} side="bottom" align="center" /></Group> : null}
        {has('fill') ? <Group><FillButton {...props} /></Group> : null}
        {has('strokeWidth') ? <Group><StrokeWidthPicker {...props} /></Group> : null}
        {has('text') ? <Group><FontSizePicker {...props} /></Group> : null}
        {hasMoreStyles(sections) ? <Group><MoreStyles {...props} sections={sections} onPreview={onPreview} /></Group> : null}
        {eraser ? <EraserControls {...eraser} /> : null}
        {selectionCount ? <Group><SelectionActions onCrop={onCrop} onDelete={onDelete} layerMoves={layerMoves} onReorder={onReorder} /></Group> : null}
      </div>
    </div>
  </div>;
}

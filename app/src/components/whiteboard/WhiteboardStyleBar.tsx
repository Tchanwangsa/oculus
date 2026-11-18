import { useRef } from 'react';
import type { LayerMove } from '@/lib/whiteboard/layers';
import type { InkPreset } from '@/lib/whiteboard/inkPresets';
import { isMarker } from '@/lib/whiteboard/pens';
import type { InspectorSection, StylePatch, StyleValues } from '@/lib/whiteboard/style';
import type { WhiteboardToolPreferences } from '@/lib/whiteboard/toolPreferences';
import { useScrollFade } from '@/hooks/ui/useScrollFade';
import { WhiteboardPresetBar } from './WhiteboardPresetBar';
import { ColorRow } from './inspector/ColorRow';
import { EraserControls } from './inspector/EraserControls';
import { FillButton } from './inspector/FillButton';
import { FontSizePicker } from './inspector/FontSizePicker';
import { hasMoreStyles, MoreStyles } from './inspector/MoreStyles';
import { PenMenu } from './inspector/PenMenu';
import { SelectionActions } from './inspector/SelectionActions';
import { StrokeWidthControl } from './inspector/StrokeWidthControl';
import { StyleGroup as Group } from './inspector/StyleGroup';
import { MARKER_PICKS, STROKE_PALETTE, STROKE_PICKS } from './inspector/palette';
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
  /** With the pen or highlighter and nothing selected: its presets, one tap switching the whole look. */
  presets?: { items: InkPreset[]; active: number; onSelect: (at: number) => void; label: string } | null;
  /** With the eraser chosen: its device-local settings, shown in place of styles. */
  eraser?: { preferences: WhiteboardToolPreferences; onChange: (patch: Partial<WhiteboardToolPreferences>) => void };
}

/**
 * The style half of the whiteboard's top bar, holding only what applies: the ink tool's presets, colour, pen, fill, stroke
 * width and font size inline, the rest behind More styles, then the selection's actions; with the eraser, its settings.
 * Its popovers open below it. A pane too narrow for it scrolls it sideways with faded edges.
 */
export function WhiteboardStyleBar({ sections, values, colorLabel, onChange, onPreview, selectionCount, onDelete, onCrop, layerMoves, onReorder, presets, eraser }: StyleBarProps) {
  const scroller = useRef<HTMLDivElement>(null);
  useScrollFade(scroller, 'x');
  const has = (section: InspectorSection) => sections.includes(section);
  const props = { values, onChange };
  return <div ref={scroller} data-whiteboard-inspector="true" role="toolbar" aria-label={eraser ? 'Eraser' : 'Style'} onMouseDown={keepTextFocus}
    className="whiteboard-style-bar min-w-0 overflow-x-auto overscroll-contain">
    <div className="flex w-max items-center">
      {presets ? <Group><WhiteboardPresetBar {...presets} /></Group> : null}
      {has('stroke') ? <Group><ColorRow label={colorLabel} value={values.stroke} onChange={stroke => onChange({ stroke })} picks={has('pen') && isMarker(values.pen) ? MARKER_PICKS : STROKE_PICKS} palette={STROKE_PALETTE} side="bottom" align="start" /></Group> : null}
      {has('pen') ? <Group><PenMenu {...props} /></Group> : null}
      {has('fill') ? <Group><FillButton {...props} /></Group> : null}
      {has('strokeWidth') ? <Group><StrokeWidthControl {...props} onPreview={onPreview} className="w-40" /></Group> : null}
      {has('text') ? <Group><FontSizePicker {...props} /></Group> : null}
      {hasMoreStyles(sections) ? <Group><MoreStyles {...props} sections={sections} onPreview={onPreview} /></Group> : null}
      {eraser ? <EraserControls {...eraser} /> : null}
      {selectionCount ? <Group><SelectionActions onCrop={onCrop} onDelete={onDelete} layerMoves={layerMoves} onReorder={onReorder} /></Group> : null}
    </div>
  </div>;
}

import type { CanvasArrowhead } from '@/lib/canvas/model';
import { arrowheadShape } from '@/lib/canvas/geometry';
import { InspectorSection, type SectionProps } from './InspectorSection';
import { Segmented } from './Segmented';

const HEADS: [CanvasArrowhead, string][] = [['none', 'None'], ['arrow', 'Arrow'], ['triangle', 'Triangle'], ['dot', 'Dot'], ['bar', 'Bar'], ['diamond', 'Diamond']];
const SHAFT = [{ x: 3, y: 7 }, { x: 21, y: 7 }];

/** Drawn by the canvas's own arrowhead geometry, so a preview matches what the arrow gets. */
function ArrowheadGlyph({ type, end }: { type: CanvasArrowhead; end: 'start' | 'end' }) {
  const head = arrowheadShape(SHAFT, end, type);
  return <svg width="22" height="13" viewBox="0 0 24 14" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
    <path d="M3 7H21" />
    {head?.dot ? <circle cx={head.dot.center.x} cy={head.dot.center.y} r={head.dot.radius} fill="currentColor" stroke="none" />
      : head ? <path d={`M${head.points.map(p => `${p.x} ${p.y}`).join('L')}${head.closed ? 'Z' : ''}`} fill={head.closed ? 'currentColor' : 'none'} /> : null}
  </svg>;
}

export function ArrowheadSection({ values, onChange }: SectionProps) {
  return <InspectorSection label="Arrowheads">
    {(['start', 'end'] as const).map(end => <div key={end} className="flex items-center gap-2">
      <span className="w-7 shrink-0 text-[11px] text-muted-foreground">{end === 'start' ? 'Start' : 'End'}</span>
      <Segmented label={`${end === 'start' ? 'Start' : 'End'} arrowhead`} className="flex-1"
        value={end === 'start' ? values.startArrowhead : values.endArrowhead}
        onChange={head => onChange(end === 'start' ? { startArrowhead: head } : { endArrowhead: head })}
        options={HEADS.map(([value, label]) => ({ value, label, content: <ArrowheadGlyph type={value} end={end} /> }))} />
    </div>)}
  </InspectorSection>;
}

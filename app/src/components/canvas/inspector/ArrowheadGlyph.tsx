import type { CanvasArrowhead } from '@/lib/canvas/model';
import { arrowheadShape } from '@/lib/canvas/geometry';

const SHAFT = [{ x: 3, y: 7 }, { x: 21, y: 7 }];

/** Drawn by the canvas's own arrowhead geometry, so a preview matches what the arrow gets. */
export function ArrowheadGlyph({ type, end }: { type: CanvasArrowhead; end: 'start' | 'end' }) {
  const head = arrowheadShape(SHAFT, end, type);
  return <svg width="22" height="13" viewBox="0 0 24 14" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
    <path d="M3 7H21" />
    {head?.dot ? <circle cx={head.dot.center.x} cy={head.dot.center.y} r={head.dot.radius} fill="currentColor" stroke="none" />
      : head ? <path d={`M${head.points.map(p => `${p.x} ${p.y}`).join('L')}${head.closed ? 'Z' : ''}`} fill={head.closed ? 'currentColor' : 'none'} /> : null}
  </svg>;
}

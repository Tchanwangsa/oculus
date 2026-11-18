import { useState } from 'react';
import { Input } from '@/components/ui/input';
import { Slider } from '@/components/ui/slider';
import { cn } from '@/lib/utils';
import { isMarker } from '@/lib/canvas/pens';
import type { StylePatch } from '@/lib/canvas/style';
import type { SectionProps } from './InspectorSection';

const MIN = 0.5, STEP = 0.5;
const round = (width: number, max: number) => Math.min(max, Math.max(MIN, Math.round(width / STEP) * STEP));

/**
 * The stroke's width in world units: a slider that previews while it is dragged and a field for an exact number.
 * A half-typed number never commits; Enter or leaving the field applies it, Escape reverts. A marker reaches wider.
 */
export function StrokeWidthControl({ values, onChange, onPreview, className }: SectionProps & { onPreview: (patch: StylePatch) => void; className?: string }) {
  const max = isMarker(values.pen) ? 60 : 30;
  const [live, setLive] = useState<number | null>(null);
  const [draft, setDraft] = useState<string | null>(null);
  const width = live ?? values.strokeWidth;
  const commit = () => {
    if (draft === null) return;
    setDraft(null);
    const typed = Number.parseFloat(draft);
    if (Number.isFinite(typed) && typed > 0) onChange({ strokeWidth: round(typed, max) });
  };
  return <div className={cn('flex min-w-0 items-center gap-2', className)}>
    <Slider aria-label="Stroke width" min={MIN} max={max} step={STEP} value={[Math.min(max, width)]} className="min-w-0 flex-1 py-1.5"
      onValueChange={([value]) => { setLive(value); onPreview({ strokeWidth: value }); }}
      onValueCommit={([value]) => { setLive(null); onChange({ strokeWidth: value }); }} />
    <Input aria-label="Stroke width in points" inputMode="decimal" value={draft ?? String(width)}
      onChange={event => setDraft(event.target.value)} onFocus={event => event.currentTarget.select()} onBlur={commit}
      onKeyDown={event => {
        if (event.key === 'Enter') { event.preventDefault(); commit(); }
        else if (event.key === 'Escape') { event.preventDefault(); setDraft(null); event.currentTarget.blur(); }
      }}
      className="h-8 w-12 shrink-0 rounded-[10px] px-1.5 text-center text-[12px] tabular-nums shadow-none" />
  </div>;
}

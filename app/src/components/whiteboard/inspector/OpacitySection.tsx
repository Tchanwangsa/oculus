import { useState } from 'react';
import { Slider } from '@/components/ui/slider';
import type { StylePatch } from '@/lib/canvas/style';
import { InspectorSection, type SectionProps } from './InspectorSection';

/** Dragging previews on the canvas; releasing commits one undo step. */
export function OpacitySection({ values, onChange, onPreview }: SectionProps & { onPreview: (patch: StylePatch) => void }) {
  const [live, setLive] = useState<number | null>(null);
  const percent = live ?? Math.round(values.opacity * 100);
  return <InspectorSection label="Opacity" aside={<span className="tabular-nums text-foreground">{percent}%</span>}>
    <Slider aria-label="Opacity" min={0} max={100} step={1} value={[percent]} className="py-1.5"
      onValueChange={([value]) => { setLive(value); onPreview({ opacity: value / 100 }); }}
      onValueCommit={([value]) => { setLive(null); onChange({ opacity: value / 100 }); }} />
  </InspectorSection>;
}

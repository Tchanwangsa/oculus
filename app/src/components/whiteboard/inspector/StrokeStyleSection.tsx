import type { CanvasStrokeStyle } from '@/lib/canvas/model';
import { InspectorSection, type SectionProps } from './InspectorSection';
import { Segmented } from './Segmented';

const STYLES: [CanvasStrokeStyle, string, string | undefined][] = [['solid', 'Solid', undefined], ['dashed', 'Dashed', '5 3.5'], ['dotted', 'Dotted', '0 3.5']];

export function StrokeStyleSection({ values, onChange }: SectionProps) {
  return <InspectorSection label="Stroke style">
    <Segmented label="Stroke style" value={values.strokeStyle} onChange={strokeStyle => onChange({ strokeStyle })}
      options={STYLES.map(([value, label, dash]) => ({ value, label, content: <svg width="26" height="8" viewBox="0 0 26 8" aria-hidden="true"><path d="M3 4H23" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeDasharray={dash} /></svg> }))} />
  </InspectorSection>;
}

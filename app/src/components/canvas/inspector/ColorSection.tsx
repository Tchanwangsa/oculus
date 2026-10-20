import { InspectorSection, type SectionProps } from './InspectorSection';
import { ColorRow } from './ColorRow';
import { STROKE_PALETTE, STROKE_PICKS } from './palette';

export function ColorSection({ values, onChange, label }: SectionProps & { label: string }) {
  return <InspectorSection label={label}>
    <ColorRow label={label} value={values.stroke} onChange={stroke => onChange({ stroke })} picks={STROKE_PICKS} palette={STROKE_PALETTE} />
  </InspectorSection>;
}

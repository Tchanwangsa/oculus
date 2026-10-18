import { InspectorSection, type SectionProps } from './InspectorSection';
import { ColorRow } from './ColorRow';
import { FILL_PALETTE, FILL_PICKS } from './palette';

export function FillSection({ values, onChange }: SectionProps) {
  return <InspectorSection label="Fill">
    <ColorRow label="Fill" value={values.fill} onChange={fill => onChange({ fill })} picks={FILL_PICKS} palette={FILL_PALETTE} />
  </InspectorSection>;
}

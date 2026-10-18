import { InspectorSection, type SectionProps } from './InspectorSection';
import { fills, Swatches } from './Swatches';

export function FillSection({ values, onChange }: SectionProps) {
  return <InspectorSection label="Fill">
    <Swatches label="Fill" colors={fills} value={values.fill} onChange={fill => onChange({ fill })} />
  </InspectorSection>;
}

import { ArrowBendUpRightIcon, ArrowElbowRightUpIcon, ArrowUpRightIcon } from '@phosphor-icons/react';
import { InspectorSection, type SectionProps } from './InspectorSection';
import { Segmented } from './Segmented';

export function ArrowTypeSection({ values, onChange }: SectionProps) {
  return <InspectorSection label="Arrow type">
    <Segmented label="Arrow type" value={values.arrowType} onChange={arrowType => onChange({ arrowType })} options={[
      { value: 'sharp', label: 'Sharp', content: <ArrowUpRightIcon size={16} /> },
      { value: 'curved', label: 'Curved', content: <ArrowBendUpRightIcon size={16} /> },
      { value: 'elbow', label: 'Elbow', content: <ArrowElbowRightUpIcon size={16} /> },
    ]} />
  </InspectorSection>;
}

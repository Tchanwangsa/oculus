import { PencilSimpleIcon, WaveSineIcon } from '@phosphor-icons/react';
import { InspectorSection, type SectionProps } from './InspectorSection';
import { Segmented } from './Segmented';

export function SketchSection({ values, onChange }: SectionProps) {
  return <InspectorSection label="Line style">
    <Segmented label="Line style" value={values.style} onChange={style => onChange({ style })} options={[
      { value: 'sketch', label: 'Sketch', content: <WaveSineIcon size={19} /> },
      { value: 'clean', label: 'Clean', content: <PencilSimpleIcon size={16} /> },
    ]} />
  </InspectorSection>;
}

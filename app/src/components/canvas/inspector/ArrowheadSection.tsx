import { InspectorSection, type SectionProps } from './InspectorSection';
import { ArrowheadPicker } from './ArrowheadPicker';

export function ArrowheadSection({ values, onChange }: SectionProps) {
  return <InspectorSection label="Arrowheads">
    <div className="flex gap-1.5">
      <ArrowheadPicker end="start" value={values.startArrowhead} onChange={startArrowhead => onChange({ startArrowhead })} />
      <ArrowheadPicker end="end" value={values.endArrowhead} onChange={endArrowhead => onChange({ endArrowhead })} />
    </div>
  </InspectorSection>;
}

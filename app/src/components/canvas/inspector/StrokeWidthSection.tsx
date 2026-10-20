import { InspectorSection, type SectionProps } from './InspectorSection';
import { Segmented } from './Segmented';

const WIDTHS: [number, string][] = [[1, 'Thin'], [2, 'Medium'], [4, 'Bold']];

export function StrokeWidthSection({ values, onChange }: SectionProps) {
  return <InspectorSection label="Stroke width">
    <Segmented label="Stroke width" value={values.strokeWidth} onChange={strokeWidth => onChange({ strokeWidth })}
      options={WIDTHS.map(([width, name]) => ({ value: width, label: name, content: <span className="w-6 rounded-full bg-current" style={{ height: width }} /> }))} />
  </InspectorSection>;
}

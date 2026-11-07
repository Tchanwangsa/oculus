import type { SectionProps } from './InspectorSection';
import { Segmented } from './Segmented';

const WIDTHS: [number, string][] = [[1, 'Thin'], [2, 'Medium'], [4, 'Bold']];

/** The three widths as lines of that weight, at a fixed width for the style bar. */
export function StrokeWidthPicker({ values, onChange }: SectionProps) {
  return <Segmented label="Stroke width" className="w-28 shrink-0" value={values.strokeWidth} onChange={strokeWidth => onChange({ strokeWidth })}
    options={WIDTHS.map(([width, name]) => ({ value: width, label: name, content: <span className="w-6 rounded-full bg-current" style={{ height: width }} /> }))} />;
}

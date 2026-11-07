import type { SectionProps } from './InspectorSection';
import { FontSizeField } from './FontSizeField';
import { Segmented } from './Segmented';

const SIZES: [number, string, string][] = [[16, 'S', 'Small'], [24, 'M', 'Medium'], [36, 'L', 'Large'], [48, 'XL', 'Extra large']];

/** Four preset sizes and the exact size as a field; narrow panes drop the field (`canvas.css`). */
export function FontSizePicker({ values, onChange }: SectionProps) {
  return <div className="flex items-center gap-1.5">
    <Segmented label="Font size" className="w-[7.5rem] shrink-0" value={values.fontSize} onChange={fontSize => onChange({ fontSize })}
      options={SIZES.map(([value, short, label]) => ({ value, label: `${label} (${value})`, content: <span className="text-[12px] font-medium">{short}</span> }))} />
    <div className="canvas-style-size-field flex"><FontSizeField value={values.fontSize} onChange={fontSize => onChange({ fontSize })} /></div>
  </div>;
}

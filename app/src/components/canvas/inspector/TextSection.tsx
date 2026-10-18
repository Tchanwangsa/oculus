import { AlignBottomSimpleIcon, AlignCenterVerticalSimpleIcon, AlignTopSimpleIcon, TextAlignCenterIcon, TextAlignLeftIcon, TextAlignRightIcon } from '@phosphor-icons/react';
import type { CanvasFontFamily } from '@/lib/canvas/model';
import { CANVAS_FONT_STACKS } from '@/lib/canvas/renderText';
import { InspectorSection, type SectionProps } from './InspectorSection';
import { FontSizeField } from './FontSizeField';
import { Segmented } from './Segmented';

const FAMILIES: [CanvasFontFamily, string][] = [['sans', 'Sans'], ['serif', 'Serif'], ['mono', 'Mono']];
const SIZES: [number, string, string][] = [[16, 'S', 'Small'], [24, 'M', 'Medium'], [36, 'L', 'Large'], [48, 'XL', 'Extra large']];

/** Font, size and alignment; `vertical` adds top/middle/bottom for shape labels. */
export function TextSection({ values, onChange, vertical }: SectionProps & { vertical: boolean }) {
  return <>
    <InspectorSection label="Font">
      <Segmented label="Font family" value={values.fontFamily} onChange={fontFamily => onChange({ fontFamily })}
        options={FAMILIES.map(([value, label]) => ({ value, label, content: <span className="text-[14px] leading-none" style={{ fontFamily: CANVAS_FONT_STACKS[value] }}>Aa</span> }))} />
      <div className="flex items-center gap-1.5">
        <Segmented label="Font size" className="flex-1" value={values.fontSize} onChange={fontSize => onChange({ fontSize })}
          options={SIZES.map(([value, short, label]) => ({ value, label: `${label} (${value})`, content: <span className="text-[12px] font-medium">{short}</span> }))} />
        <FontSizeField value={values.fontSize} onChange={fontSize => onChange({ fontSize })} />
      </div>
    </InspectorSection>
    <InspectorSection label="Align">
      <div className="flex items-center gap-1.5">
        <Segmented label="Text alignment" className="flex-1" value={values.textAlign} onChange={textAlign => onChange({ textAlign })} options={[
          { value: 'left', label: 'Align left', content: <TextAlignLeftIcon size={16} /> },
          { value: 'center', label: 'Align centre', content: <TextAlignCenterIcon size={16} /> },
          { value: 'right', label: 'Align right', content: <TextAlignRightIcon size={16} /> },
        ]} />
        {vertical ? <Segmented label="Vertical alignment" className="flex-1" value={values.verticalAlign} onChange={verticalAlign => onChange({ verticalAlign })} options={[
          { value: 'top', label: 'Align top', content: <AlignTopSimpleIcon size={16} /> },
          { value: 'middle', label: 'Align middle', content: <AlignCenterVerticalSimpleIcon size={16} /> },
          { value: 'bottom', label: 'Align bottom', content: <AlignBottomSimpleIcon size={16} /> },
        ]} /> : null}
      </div>
    </InspectorSection>
  </>;
}

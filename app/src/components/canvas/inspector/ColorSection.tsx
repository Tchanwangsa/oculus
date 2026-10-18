import { ColorField } from '@/components/ui/color/ColorField';
import { InspectorSection, type SectionProps } from './InspectorSection';
import { strokes, Swatches } from './Swatches';

export function ColorSection({ values, onChange, label }: SectionProps & { label: string }) {
  return <InspectorSection label={label}>
    <Swatches label={label} colors={strokes} value={values.stroke} onChange={stroke => onChange({ stroke })} dark />
    <ColorField align="start" side="right" label={`Custom ${label.toLowerCase()} color`} value={values.stroke} onChange={stroke => onChange({ stroke })} opaque className="text-[11px]" />
  </InspectorSection>;
}

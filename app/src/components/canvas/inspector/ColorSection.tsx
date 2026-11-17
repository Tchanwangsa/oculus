import type { PaletteColor } from '@/components/ui/color/ColorPalette';
import { InspectorSection, type SectionProps } from './InspectorSection';
import { ColorRow } from './ColorRow';
import { STROKE_PALETTE, STROKE_PICKS } from './palette';

/** `picks` replaces the stroke picks, as the highlighter's inks do. */
export function ColorSection({ values, onChange, label, picks = STROKE_PICKS }: SectionProps & { label: string; picks?: PaletteColor[] }) {
  return <InspectorSection label={label}>
    <ColorRow label={label} value={values.stroke} onChange={stroke => onChange({ stroke })} picks={picks} palette={STROKE_PALETTE} wrap />
  </InspectorSection>;
}

import { InspectorSection, type SectionProps } from './InspectorSection';
import { Segmented } from './Segmented';

/** A corner of a box, drawn the way each choice draws one. */
const corner = (d: string) => <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d={d} /></svg>;

export function RoundnessSection({ values, onChange }: SectionProps) {
  return <InspectorSection label="Edges">
    <Segmented label="Edges" value={values.roundness} onChange={roundness => onChange({ roundness })} options={[
      { value: 'sharp', label: 'Sharp', content: corner('M3 13V3H13') },
      { value: 'round', label: 'Round', content: corner('M3 13V8A5 5 0 0 1 8 3H13') },
    ]} />
  </InspectorSection>;
}

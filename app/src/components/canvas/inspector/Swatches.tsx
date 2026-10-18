import { CheckIcon } from '@phosphor-icons/react';
import { cn } from '@/lib/utils';

export const strokes = [
  ['Graphite', '#292b32'], ['Indigo', '#5e6ad2'], ['Blue', '#3b82b6'],
  ['Green', '#438b6b'], ['Amber', '#b78835'], ['Rose', '#c35b72'],
];
export const fills = [
  ['None', 'transparent'], ['Lavender', '#e6e5fb'], ['Blue', '#dcecf7'],
  ['Green', '#dff0e5'], ['Amber', '#fbefcc'], ['Rose', '#f8e0e7'],
];

/** `dark` swatches carry a white check; light fills a graphite one. */
export function Swatches({ colors, value, onChange, label, dark }: { colors: string[][]; value: string; onChange: (value: string) => void; label: string; dark?: boolean }) {
  return <div className="flex gap-1.5" role="group" aria-label={label}>
    {colors.map(([name, color]) => <button key={color} type="button" title={name} aria-label={`${label}: ${name}`} aria-pressed={value === color}
      onClick={() => onChange(color)}
      className={cn('canvas-swatch flex size-6 cursor-pointer items-center justify-center rounded-md border border-black/10 outline-none focus-visible:ring-2 focus-visible:ring-brand focus-visible:ring-offset-2', color === 'transparent' && 'canvas-no-fill', value === color && 'ring-2 ring-brand ring-offset-2 ring-offset-card')}
      style={{ backgroundColor: color }}>
      {value === color && color !== 'transparent' ? <CheckIcon size={13} weight="bold" className={dark ? 'text-white' : 'text-[#292b32]'} /> : null}
    </button>)}
  </div>;
}

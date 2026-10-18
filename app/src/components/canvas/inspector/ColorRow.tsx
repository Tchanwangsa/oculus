import { ColorPopover } from '@/components/ui/color/ColorField';
import { PaletteSwatch, swatchBackground, type PaletteColor } from '@/components/ui/color/ColorPalette';
import { inspectorPopoverProps } from './popoverGuards';

/** A few top picks, then the current colour as a button that opens the whole palette. */
export function ColorRow({ label, value, onChange, picks, palette }: {
  label: string; value: string; onChange: (color: string) => void; picks: PaletteColor[]; palette: PaletteColor[];
}) {
  const same = (color: string) => color.toLowerCase() === value.toLowerCase();
  return <div className="flex items-center">
    <div role="group" aria-label={label} className="flex gap-1">
      {picks.map(c => <PaletteSwatch key={c.name} color={c.color} name={c.name} label={label} selected={same(c.color)} onPick={onChange} className="focus-visible:ring-offset-card" />)}
    </div>
    <span aria-hidden className="mx-2 h-5 w-px bg-border" />
    <ColorPopover value={value} onChange={onChange} label={label} palette={palette} opaque side="right" align="start" contentProps={inspectorPopoverProps}
      className="rounded-md outline-none focus-visible:ring-2 focus-visible:ring-brand focus-visible:ring-offset-2 focus-visible:ring-offset-card">
      <span className="block size-7 rounded-md border border-black/10" style={{ background: swatchBackground(value) }} />
    </ColorPopover>
  </div>;
}

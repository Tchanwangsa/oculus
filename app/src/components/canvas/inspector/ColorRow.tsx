import { ColorPopover } from '@/components/ui/color/ColorField';
import { PaletteSwatch, swatchBackground, type PaletteColor } from '@/components/ui/color/ColorPalette';
import { cn } from '@/lib/utils';
import { inspectorPopoverProps } from './popoverGuards';

/** A few top picks, then the current colour as a button that opens the whole palette. A narrow top bar keeps the first five picks, then only the button (`canvas.css`). */
export function ColorRow({ label, value, onChange, picks, palette, side = 'right', wrap = false }: {
  label: string; value: string; onChange: (color: string) => void; picks: PaletteColor[]; palette: PaletteColor[]; side?: 'right' | 'bottom'; wrap?: boolean;
}) {
  const same = (color: string) => color.toLowerCase() === value.toLowerCase();
  return <div className="flex items-center">
    <div role="group" aria-label={label} className={wrap ? 'flex flex-wrap gap-1' : 'canvas-picks flex gap-1'}>
      {picks.map((c, i) => <PaletteSwatch key={c.name} color={c.color} name={c.name} label={label} selected={same(c.color)} onPick={onChange} className={cn('focus-visible:ring-offset-card', i >= 5 && 'canvas-pick-extra')} />)}
    </div>
    <span aria-hidden className={cn('mx-2 h-5 w-px bg-border', !wrap && 'canvas-picks')} />
    <ColorPopover value={value} onChange={onChange} label={label} palette={palette} opaque side={side} align="start" contentProps={inspectorPopoverProps}
      className="rounded-md outline-none focus-visible:ring-2 focus-visible:ring-brand focus-visible:ring-offset-2 focus-visible:ring-offset-card">
      <span className="block size-7 rounded-md border border-black/10" style={{ background: swatchBackground(value) }} />
    </ColorPopover>
  </div>;
}

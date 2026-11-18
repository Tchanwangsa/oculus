import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip';
import { swatchBackground } from '@/components/ui/color/ColorPalette';
import type { InkPreset } from '@/lib/whiteboard/inkPresets';
import { PEN_LABELS, isMarker } from '@/lib/whiteboard/pens';
import { cn } from '@/lib/utils';

/** A preset as a dot of its ink: its colour and opacity, and its width as the dot's size (a marker's as a bar). */
function PresetDot({ preset }: { preset: InkPreset }) {
  const marker = isMarker(preset.pen), size = Math.round(Math.min(18, Math.max(6, marker ? preset.strokeWidth * 0.7 : 4 + preset.strokeWidth * 1.6)));
  return <span className={cn('block border border-black/10', marker ? 'rounded-[3px]' : 'rounded-full')}
    style={{ width: marker ? 18 : size, height: size, background: swatchBackground(preset.stroke), opacity: Math.max(0.25, preset.opacity) }} />;
}

/** The ink tool's presets: one tap switches pen, colour, width and opacity together. Changing the look edits the one in use. */
export function WhiteboardPresetBar({ items, active, onSelect, label }: { items: InkPreset[]; active: number; onSelect: (at: number) => void; label: string }) {
  return <div role="group" aria-label={`${label} presets`} className="flex shrink-0 items-center gap-0.5 rounded-[10px] bg-muted p-0.5">
    {items.map((preset, i) => <Tooltip key={i}>
      <TooltipTrigger asChild>
        <button type="button" aria-label={`${label} ${i + 1}: ${PEN_LABELS[preset.pen]}, width ${preset.strokeWidth}`} aria-pressed={i === active} onClick={() => onSelect(i)}
          className={cn('flex size-7 cursor-pointer items-center justify-center rounded-lg outline-none transition-[background-color,box-shadow] focus-visible:ring-2 focus-visible:ring-brand', i === active && 'bg-card shadow-xs')}>
          <PresetDot preset={preset} />
        </button>
      </TooltipTrigger>
      <TooltipContent side="bottom" sideOffset={6}>{label} {i + 1} · {PEN_LABELS[preset.pen]} · {preset.strokeWidth}</TooltipContent>
    </Tooltip>)}
  </div>;
}

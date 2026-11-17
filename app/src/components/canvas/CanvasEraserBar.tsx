import { Slider } from '@/components/ui/slider';
import { ERASER_SIZES, type CanvasToolPreferences } from '@/lib/canvas/toolPreferences';
import { Segmented } from './inspector/Segmented';

/** The eraser's half of the top bar: whole strokes or parts of them, and how wide the partial eraser is. Saved on this device. */
export function CanvasEraserBar({ preferences, onChange }: { preferences: CanvasToolPreferences; onChange: (preferences: CanvasToolPreferences) => void }) {
  const partial = preferences.eraserMode === 'partial';
  return <div className="flex shrink-0 items-center gap-2.5">
    <Segmented label="Eraser" value={preferences.eraserMode} onChange={eraserMode => onChange({ ...preferences, eraserMode })} className="w-44"
      options={[
        { value: 'stroke', label: 'Erase whole strokes and shapes', content: <span className="text-[11px]">Whole stroke</span> },
        { value: 'partial', label: 'Rub out part of a pen or highlighter stroke', content: <span className="text-[11px]">Partial</span> },
      ]} />
    {partial ? <>
      <span aria-hidden className="h-5 w-px shrink-0 bg-border" />
      <Slider aria-label="Eraser size" min={ERASER_SIZES.min} max={ERASER_SIZES.max} step={2} value={[preferences.eraserSize]} className="w-28 py-1.5"
        onValueChange={([eraserSize]) => onChange({ ...preferences, eraserSize })} />
      <span className="w-6 text-[12px] tabular-nums text-muted-foreground">{preferences.eraserSize}</span>
    </> : null}
  </div>;
}

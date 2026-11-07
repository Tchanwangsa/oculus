import { CAPTURE_LIFT_DELAY_MS, type CaptureHoldIndicator } from '@/lib/canvas/region';
import type { CanvasViewport } from '@/lib/canvas/model';
import { SelectionPlusIcon } from '@phosphor-icons/react';

export function CanvasCaptureHold({ hold, viewport }: { hold: CaptureHoldIndicator; viewport: CanvasViewport }) {
  const lifted = hold.phase === 'lifted';
  return <div role="status" aria-label={lifted ? 'Copy lifted; drag to place' : 'Hold to duplicate'} className={"canvas-capture-hold pointer-events-none absolute z-30 flex items-center gap-2 rounded-full border border-border/80 bg-card/95 px-2.5 py-1.5 text-[11px] text-foreground shadow-md backdrop-blur-md " + (lifted ? 'canvas-capture-lifted' : '')} style={{ left: 'clamp(12px, ' + (hold.point.x * viewport.zoom + viewport.x + 16) + 'px, calc(100% - 196px))', top: 'clamp(84px, ' + (hold.point.y * viewport.zoom + viewport.y - 48) + 'px, calc(100% - 96px))' }}>
    {lifted ? <SelectionPlusIcon size={22} className="text-brand" /> : <svg width="24" height="24" viewBox="0 0 24 24" aria-hidden="true" className="-rotate-90 text-brand"><circle cx="12" cy="12" r="9" fill="none" stroke="currentColor" strokeOpacity=".2" strokeWidth="2" /><circle className="canvas-capture-hold-progress" cx="12" cy="12" r="9" fill="none" stroke="currentColor" strokeWidth="2" pathLength="100" strokeDasharray="100" style={{ animationDuration: CAPTURE_LIFT_DELAY_MS + 'ms' }} /></svg>}
    <span>{lifted ? 'Drag to place copy' : 'Hold to duplicate'}</span>
  </div>;
}

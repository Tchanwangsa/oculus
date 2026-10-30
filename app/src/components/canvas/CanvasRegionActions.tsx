import { CopyIcon, SpinnerGapIcon } from '@phosphor-icons/react';
import { Button } from '@/components/ui/button';
import type { CanvasViewport } from '@/lib/canvas/model';
import { type CanvasRegion } from '@/lib/canvas/region';

export function CanvasRegionActions({ region, viewport, onCopy, copyState, copyError }: {
  region: CanvasRegion;
  viewport: CanvasViewport;
  onCopy: () => void;
  copyState: 'idle' | 'copying' | 'copied' | 'duplicating';
  copyError: string | null;
}) {
  if (region.kind === 'capture') {
    const { bounds } = region, top = bounds.y * viewport.zoom + viewport.y, bottom = (bounds.y + bounds.height) * viewport.zoom + viewport.y;
    const copying = copyState === 'copying' || copyState === 'duplicating';
    return <div data-canvas-inspector="true" className="absolute z-30 -translate-x-1/2" style={{ left: "clamp(64px, " + ((bounds.x + bounds.width / 2) * viewport.zoom + viewport.x) + "px, calc(100% - 64px))", top: "clamp(84px, " + (top >= 130 ? top - 44 : bottom + 12) + "px, calc(100% - 108px))" }}>
      <Button variant="outline" size="sm" className="gap-2 border-border/80 bg-card/95 text-foreground shadow-md backdrop-blur-md hover:bg-card dark:border-border/80 dark:bg-card/95 dark:hover:bg-card disabled:opacity-100" disabled={copying} onClick={onCopy} aria-label="Copy capture">{copying ? <SpinnerGapIcon className="animate-spin" /> : <CopyIcon size={16} />}{copyState === 'copied' ? 'Copied' : 'Copy'}</Button>
      {copyError ? <p role="alert" className="mt-1 max-w-64 rounded-md border border-border-subtle bg-card p-2 text-xs text-destructive">{copyError}</p> : null}
    </div>;
  }
  return copyError ? <p role="alert" className="absolute top-[5.5rem] right-4 z-30 max-w-72 rounded-md border border-border-subtle bg-card p-2 text-xs text-destructive">{copyError}</p> : null;
}

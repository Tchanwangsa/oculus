import { CopyIcon, SelectionPlusIcon, SpinnerGapIcon } from '@phosphor-icons/react';
import { Button } from '@/components/ui/button';
import type { WhiteboardViewport } from '@/lib/whiteboard/model';
import { type WhiteboardRegion } from '@/lib/whiteboard/region';
import type { DrawingKind } from '@/lib/whiteboard/drawingKinds';
import { kindNoun } from '@/lib/whiteboard/kindWords';

export function WhiteboardRegionActions({ region, viewport, kind, onCopy, onDuplicate, copyState, copyError }: {
  region: WhiteboardRegion;
  kind: DrawingKind;
  viewport: WhiteboardViewport;
  onCopy: () => void;
  onDuplicate: () => void;
  copyState: 'idle' | 'copying' | 'copied' | 'duplicating';
  copyError: string | null;
}) {
  if (region.kind === 'capture') {
    const { bounds } = region, top = bounds.y * viewport.zoom + viewport.y, bottom = (bounds.y + bounds.height) * viewport.zoom + viewport.y;
    const copying = copyState === 'copying' || copyState === 'duplicating';
    return <div data-whiteboard-inspector="true" className="absolute z-30 -translate-x-1/2" style={{ left: "clamp(116px, " + ((bounds.x + bounds.width / 2) * viewport.zoom + viewport.x) + "px, calc(100% - 116px))", top: "clamp(12px, " + (top >= 56 ? top - 44 : bottom + 12) + "px, calc(100% - 64px))" }}>
      <div className="inline-flex items-center rounded-full border border-border/80 bg-card/95 p-0.5 text-foreground shadow-md backdrop-blur-md">
        <Button variant="ghost" size="sm" className="gap-2 disabled:opacity-100" disabled={copying} onClick={onCopy} aria-label="Copy capture">{copyState === 'copying' ? <SpinnerGapIcon className="animate-spin" /> : <CopyIcon size={16} />}{copyState === 'copied' ? 'Copied' : 'Copy'}</Button>
        <span className="h-4 border-l border-border/80" aria-hidden="true" />
        <Button variant="ghost" size="sm" className="gap-2 disabled:opacity-100" disabled={copying} onClick={onDuplicate} aria-label="Duplicate capture" title={`Duplicate on the ${kindNoun(kind)}; or hold inside the selection to lift a copy`}>{copyState === 'duplicating' ? <SpinnerGapIcon className="animate-spin" /> : <SelectionPlusIcon size={16} />}Duplicate</Button>
      </div>
      {copyError ? <p role="alert" className="mt-1 max-w-64 rounded-md border border-border-subtle bg-card p-2 text-xs text-destructive">{copyError}</p> : null}
    </div>;
  }
  return copyError ? <p role="alert" className="absolute top-3 right-3 z-30 max-w-72 rounded-md border border-border-subtle bg-card p-2 text-xs text-destructive">{copyError}</p> : null;
}

import { useState } from 'react';
import { CropIcon, StackIcon, TrashIcon } from '@phosphor-icons/react';
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover';
import type { LayerMove } from '@/lib/canvas/layers';
import { CanvasIconButton } from '../CanvasToolbar';
import { LayerSection } from './LayerSection';
import { inspectorPopoverProps, keepTextFocus } from './popoverGuards';

/** What can be done to the selection: crop a lone image, move it through the layers (in a popover), delete it. */
export function SelectionActions({ onCrop, onDelete, layerMoves, onReorder }: { onCrop?: () => void; onDelete: () => void; layerMoves: LayerMove[]; onReorder: (move: LayerMove) => void }) {
  const [layerOpen, setLayerOpen] = useState(false);
  return <>
    {onCrop ? <CanvasIconButton label="Crop image" onClick={onCrop} className="size-8"><CropIcon size={17} /></CanvasIconButton> : null}
    <Popover open={layerOpen} onOpenChange={setLayerOpen}>
      <PopoverTrigger asChild>
        <CanvasIconButton label="Layer" active={layerOpen} className="size-8"><StackIcon size={17} /></CanvasIconButton>
      </PopoverTrigger>
      <PopoverContent {...inspectorPopoverProps} onMouseDown={keepTextFocus} side="bottom" align="end" sideOffset={8} collisionPadding={12} className="canvas-chrome w-[184px] p-3 text-xs">
        <LayerSection moves={layerMoves} onReorder={onReorder} />
      </PopoverContent>
    </Popover>
    <CanvasIconButton label="Delete selection" onClick={onDelete} className="size-8 hover:text-destructive"><TrashIcon size={17} /></CanvasIconButton>
  </>;
}

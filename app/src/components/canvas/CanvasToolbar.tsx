import { ArrowRightIcon, ArrowUUpLeftIcon, ArrowUUpRightIcon, CircleIcon, CursorIcon, DiamondIcon, EraserIcon, HandIcon, ImageIcon, LineSegmentIcon, PencilSimpleIcon, RectangleIcon, TextTIcon, type Icon } from '@phosphor-icons/react';
import { Button } from '@/components/ui/button';
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip';
import type { CanvasTool } from '@/lib/canvas/model';
import { shortcut } from '@/lib/platform';
import { cn } from '@/lib/utils';
import type { ReactNode, ComponentProps } from 'react';

export const canvasTools: { tool: CanvasTool; label: string; key: string; icon: Icon }[] = [
  { tool: 'select', label: 'Select', key: 'V', icon: CursorIcon },
  { tool: 'hand', label: 'Hand', key: 'H', icon: HandIcon },
  { tool: 'rectangle', label: 'Rectangle', key: 'R', icon: RectangleIcon },
  { tool: 'ellipse', label: 'Ellipse', key: 'O', icon: CircleIcon },
  { tool: 'diamond', label: 'Diamond', key: 'D', icon: DiamondIcon },
  { tool: 'arrow', label: 'Arrow', key: 'A', icon: ArrowRightIcon },
  { tool: 'line', label: 'Line', key: 'L', icon: LineSegmentIcon },
  { tool: 'freedraw', label: 'Draw', key: 'P', icon: PencilSimpleIcon },
  { tool: 'text', label: 'Text', key: 'T', icon: TextTIcon },
  { tool: 'eraser', label: 'Eraser', key: 'E', icon: EraserIcon },
];

export function CanvasIconButton({ label, children, active, className, ...props }: ComponentProps<typeof Button> & { label: string; active?: boolean; children: ReactNode }) {
  return <Tooltip>
    <TooltipTrigger asChild>
      <Button type="button" variant="ghost" size="icon" aria-label={label} className={cn('rounded-lg text-muted-foreground', active && 'bg-brand/12 text-brand hover:bg-brand/18 hover:text-brand', className)} {...props}>{children}</Button>
    </TooltipTrigger>
    <TooltipContent side="bottom" sideOffset={8}>{label}</TooltipContent>
  </Tooltip>;
}

export function CanvasToolbar({ tool, onTool, onAddImage }: { tool: CanvasTool; onTool: (tool: CanvasTool) => void; onAddImage: () => void }) {
  return <div className="canvas-toolbox absolute top-4 left-1/2 z-10 flex max-w-[calc(100%-2rem)] -translate-x-1/2 items-center gap-0.5 overflow-x-auto rounded-2xl border border-border-subtle bg-card p-1.5 shadow-sm" role="toolbar" aria-label="Drawing tools">
    {canvasTools.map(({ tool: value, label, key, icon: Glyph }, index) => <div key={value} className={cn('flex', index === 2 && 'ml-1 border-l border-border-subtle pl-1')}>
      <CanvasIconButton label={`${label} (${key})`} active={tool === value} aria-pressed={tool === value} onClick={() => onTool(value)} className="relative size-10">
        <Glyph size={19} weight={tool === value ? 'fill' : 'regular'} />
        <span className="absolute right-1.5 bottom-0.5 text-[8px] leading-none opacity-45" aria-hidden="true">{key.toLowerCase()}</span>
      </CanvasIconButton>
    </div>)}
    <div className="ml-1 flex border-l border-border-subtle pl-1">
      <CanvasIconButton label="Add image" onClick={onAddImage} className="size-10"><ImageIcon size={19} /></CanvasIconButton>
    </div>
  </div>;
}

export function CanvasHistory({ undo, redo, canUndo, canRedo }: { undo: () => void; redo: () => void; canUndo: boolean; canRedo: boolean }) {
  return <div className="flex items-center gap-0.5 rounded-xl border border-border-subtle bg-card p-1 shadow-xs">
    <CanvasIconButton label={`Undo (${shortcut('mod+Z')})`} onClick={undo} disabled={!canUndo}><ArrowUUpLeftIcon size={17} /></CanvasIconButton>
    <CanvasIconButton label={`Redo (${shortcut('mod+shift+Z')})`} onClick={redo} disabled={!canRedo}><ArrowUUpRightIcon size={17} /></CanvasIconButton>
  </div>;
}


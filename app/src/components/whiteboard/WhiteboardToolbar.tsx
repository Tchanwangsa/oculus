import { ArrowUUpLeftIcon, ArrowUUpRightIcon } from '@phosphor-icons/react';
import { Button } from '@/components/ui/button';
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip';
import type { WhiteboardTool } from '@/lib/whiteboard/model';
import type { WhiteboardTemplate } from '@/lib/whiteboard/templates';
import { shortcut } from '@/lib/platform';
import { cn } from '@/lib/utils';
import type { ReactNode, ComponentProps } from 'react';
import { ToolArt } from './toolArt/ToolArt';
import type { ToolArtName } from './toolArt/types';

/** A rail entry is a drawing tool, or an action that drops something onto the whiteboard and returns to Select. */
type RailEntry = { art: ToolArtName; label: string; key: string } & ({ tool: WhiteboardTool } | { insert: 'image' | WhiteboardTemplate });

/** The rail's groups, top to bottom: pointers, ink, things to insert, shapes, graphs. The hook's key map matches these keys. */
const railGroups: RailEntry[][] = [
  [
    { tool: 'select', art: 'select', label: 'Select', key: 'S' },
    { tool: 'lasso', art: 'lasso', label: 'Lasso', key: 'L' },
    { tool: 'hand', art: 'grab', label: 'Grab', key: 'G' },
    { tool: 'capture', art: 'capture', label: 'Capture area', key: 'C' },
  ],
  [
    { tool: 'freedraw', art: 'pen', label: 'Pen', key: 'P' },
    { tool: 'highlighter', art: 'highlighter', label: 'Highlighter', key: 'H' },
    { tool: 'eraser', art: 'eraser', label: 'Eraser', key: 'E' },
  ],
  [
    { tool: 'text', art: 'text', label: 'Text', key: 'T' },
    { insert: 'image', art: 'image', label: 'Image', key: 'I' },
    { insert: 'matrix', art: 'matrix', label: 'Matrix', key: 'M' },
  ],
  [
    { tool: 'rectangle', art: 'rectangle', label: 'Rectangle', key: 'R' },
    { tool: 'ellipse', art: 'oval', label: 'Oval', key: 'O' },
    { tool: 'diamond', art: 'diamond', label: 'Diamond', key: 'D' },
    { tool: 'arrow', art: 'arrow', label: 'Arrow', key: 'A' },
    { tool: 'line', art: 'line', label: 'Line', key: 'N' },
  ],
  [
    { insert: 'axes2', art: 'xy', label: 'X-Y axes', key: 'Y' },
    { insert: 'axes3', art: 'xyz', label: 'X-Y-Z axes', key: 'Z' },
  ],
];

export function WhiteboardIconButton({ label, children, active, className, side = 'bottom', ...props }: ComponentProps<typeof Button> & { label: string; active?: boolean; side?: 'top' | 'bottom' | 'right'; children: ReactNode }) {
  return <Tooltip>
    <TooltipTrigger asChild>
      <Button type="button" variant="ghost" size="icon" aria-label={label} className={cn('rounded-lg text-muted-foreground', active && 'bg-brand/12 text-brand hover:bg-brand/18 hover:text-brand', className)} {...props}>{children}</Button>
    </TooltipTrigger>
    <TooltipContent side={side} sideOffset={8}>{label}</TooltipContent>
  </Tooltip>;
}

/**
 * The tools as a column docked on the drawing's left edge, each drawn in its own colours when chosen (`toolArt/`); it
 * scrolls when the pane is shorter than the column.
 */
export function WhiteboardToolbar({ tool, onTool, onAddImage, onInsert }: { tool: WhiteboardTool; onTool: (tool: WhiteboardTool) => void; onAddImage: () => void; onInsert: (template: WhiteboardTemplate) => void }) {
  return <div className="whiteboard-rail flex w-(--whiteboard-rail-width) shrink-0 flex-col items-center gap-0.5 overflow-y-auto border-r border-border-subtle bg-card py-1.5" role="toolbar" aria-orientation="vertical" aria-label="Drawing tools">
    {railGroups.map((group, index) => <div key={index} className={cn('flex shrink-0 flex-col items-center gap-0.5', index > 0 && 'mt-1 border-t border-border-subtle pt-1')}>
      {group.map(entry => {
        const selected = 'tool' in entry && tool === entry.tool;
        const press = () => { if ('tool' in entry) onTool(entry.tool); else if (entry.insert === 'image') onAddImage(); else onInsert(entry.insert); };
        return <WhiteboardIconButton key={entry.label} side="right" label={`${entry.label} (${entry.key})`} aria-pressed={'tool' in entry ? selected : undefined} onClick={press}
          className={cn('relative size-10 [--tool-fill:var(--color-card)] hover:[--tool-fill:var(--color-accent)]', selected && 'bg-brand/12 hover:bg-brand/18')}>
          <ToolArt name={entry.art} colour={selected} className="size-7" />
          <span className="absolute right-1.5 bottom-0.5 text-[8px] leading-none opacity-45" aria-hidden="true">{entry.key.toLowerCase()}</span>
        </WhiteboardIconButton>;
      })}
    </div>)}
  </div>;
}

export function WhiteboardHistory({ undo, redo, canUndo, canRedo }: { undo: () => void; redo: () => void; canUndo: boolean; canRedo: boolean }) {
  return <div className="flex shrink-0 items-center gap-0.5" role="group" aria-label="History">
    <WhiteboardIconButton label={`Undo (${shortcut('mod+Z')})`} onClick={undo} disabled={!canUndo}><ArrowUUpLeftIcon size={17} /></WhiteboardIconButton>
    <WhiteboardIconButton label={`Redo (${shortcut('mod+shift+Z')})`} onClick={redo} disabled={!canRedo}><ArrowUUpRightIcon size={17} /></WhiteboardIconButton>
  </div>;
}

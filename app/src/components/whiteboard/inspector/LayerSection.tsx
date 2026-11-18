import { ArrowDownIcon, ArrowLineDownIcon, ArrowLineUpIcon, ArrowUpIcon, type Icon } from '@phosphor-icons/react';
import type { LayerMove } from '@/lib/canvas/layers';
import { shortcut } from '@/lib/platform';
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip';
import { InspectorSection } from './InspectorSection';

const MOVES: [LayerMove, string, string, Icon][] = [
  ['back', 'Send to back', 'mod+shift+[', ArrowLineDownIcon],
  ['backward', 'Send backward', 'mod+[', ArrowDownIcon],
  ['forward', 'Bring forward', 'mod+]', ArrowUpIcon],
  ['front', 'Bring to front', 'mod+shift+]', ArrowLineUpIcon],
];

/** Actions, not a choice: the track of `Segmented` without a raised segment; a move that would change nothing is disabled. */
export function LayerSection({ moves, onReorder }: { moves: LayerMove[]; onReorder: (move: LayerMove) => void }) {
  return <InspectorSection label="Layer">
    <div role="group" aria-label="Layer order" className="flex min-w-0 gap-0.5 rounded-[10px] bg-muted p-0.5">
      {MOVES.map(([move, label, keys, Glyph]) => <Tooltip key={move}>
        <TooltipTrigger asChild>
          <button type="button" aria-label={label} disabled={!moves.includes(move)} onClick={() => onReorder(move)}
            className="flex h-7 min-w-0 flex-1 cursor-pointer items-center justify-center rounded-lg text-muted-foreground transition-[color,background-color] outline-none hover:bg-card hover:text-foreground focus-visible:ring-2 focus-visible:ring-brand disabled:cursor-default disabled:bg-transparent disabled:text-muted-foreground/40">
            <Glyph size={16} />
          </button>
        </TooltipTrigger>
        <TooltipContent side="bottom" sideOffset={6}>{label} ({shortcut(keys)})</TooltipContent>
      </Tooltip>)}
    </div>
  </InspectorSection>;
}

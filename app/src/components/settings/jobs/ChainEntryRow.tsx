import type { ReactNode } from "react";
import { ArrowDown, ArrowUp, X } from "@phosphor-icons/react";

import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import type { JobSelection } from "@/lib/db";
import type { PickerProvider, Provider } from "@/lib/harness";
import { ChainPicker } from "./ChainPicker";

/** One model in the fallbacks dialog: its place in the order, its picker,
 *  and buttons to move or remove it. */
export function ChainEntryRow({
  position,
  entry,
  providers,
  isFirst,
  isLast,
  onPick,
  onView,
  onMove,
  onRemove,
}: {
  position: number;
  entry: JobSelection;
  providers: PickerProvider[];
  isFirst: boolean;
  isLast: boolean;
  onPick: (pick: JobSelection) => void;
  onView: (provider: Provider) => void;
  onMove: (by: -1 | 1) => void;
  onRemove: () => void;
}) {
  return (
    <li className="flex items-center gap-2 py-1">
      <span className="w-4 shrink-0 text-right text-xs tabular-nums text-muted-foreground">
        {position}
      </span>
      <div className="min-w-0 flex-1">
        <ChainPicker
          providers={providers}
          entry={entry}
          fallbackProvider={entry.provider}
          onPick={onPick}
          onView={onView}
        />
      </div>
      <div className="flex shrink-0 items-center gap-0.5">
        <IconAction label="Move up" disabled={isFirst} onClick={() => onMove(-1)}>
          <ArrowUp size={14} />
        </IconAction>
        <IconAction label="Move down" disabled={isLast} onClick={() => onMove(1)}>
          <ArrowDown size={14} />
        </IconAction>
        <IconAction label="Remove" onClick={onRemove}>
          <X size={14} />
        </IconAction>
      </div>
    </li>
  );
}

function IconAction({
  label,
  disabled,
  onClick,
  children,
}: {
  label: string;
  disabled?: boolean;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button
          variant="ghost"
          size="icon-xs"
          aria-label={label}
          disabled={disabled}
          onClick={onClick}
          className="text-muted-foreground hover:text-foreground"
        >
          {children}
        </Button>
      </TooltipTrigger>
      <TooltipContent>{label}</TooltipContent>
    </Tooltip>
  );
}

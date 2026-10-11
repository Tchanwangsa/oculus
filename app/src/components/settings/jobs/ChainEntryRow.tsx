import type { CSSProperties, KeyboardEvent, MouseEvent, PointerEvent, Ref } from "react";
import { DotsSixVertical, X } from "@phosphor-icons/react";

import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import type { JobSelection } from "@/lib/db";
import type { PickerProvider, Provider } from "@/lib/harness";
import { cn } from "@/lib/utils";
import { ChainPicker } from "./ChainPicker";

/** One model in the fallbacks dialog: a handle to drag it (or move it with
 *  the arrow keys), its place in the order, its picker, and a remove button.
 *  The drag itself is `ChainList`'s. */
export function ChainEntryRow({
  position,
  count,
  entry,
  providers,
  rowRef,
  handleRef,
  style,
  grabbed,
  settling,
  onPointerDown,
  onClickCapture,
  onHandleKey,
  onPick,
  onView,
  onRemove,
}: {
  position: number;
  count: number;
  entry: JobSelection;
  providers: PickerProvider[];
  rowRef: Ref<HTMLLIElement>;
  handleRef: Ref<HTMLButtonElement>;
  style: CSSProperties | undefined;
  /** This row is the one being dragged. */
  grabbed: boolean;
  /** Another row is being dragged, so this one slides out of its way. */
  settling: boolean;
  onPointerDown: (e: PointerEvent<HTMLLIElement>) => void;
  onClickCapture: (e: MouseEvent) => void;
  onHandleKey: (e: KeyboardEvent<HTMLButtonElement>) => void;
  onPick: (pick: JobSelection) => void;
  onView: (provider: Provider) => void;
  onRemove: () => void;
}) {
  return (
    <li
      ref={rowRef}
      onPointerDown={onPointerDown}
      onClickCapture={onClickCapture}
      style={style}
      className={cn(
        "group flex items-center gap-1.5 bg-card py-1 pr-1.5 pl-1 first:rounded-t-lg last:rounded-b-lg",
        grabbed
          ? "relative z-10 rounded-lg shadow-md"
          : settling && "transition-transform duration-200 ease-out",
      )}
    >
      <button
        ref={handleRef}
        type="button"
        aria-label={`Position ${position} of ${count}: the arrow keys move it`}
        onKeyDown={onHandleKey}
        className={cn(
          "flex size-6 shrink-0 items-center justify-center rounded-md text-muted-foreground/70",
          "cursor-grab outline-none hover:text-foreground focus-visible:ring-[3px] focus-visible:ring-ring/50",
          grabbed && "cursor-grabbing",
        )}
      >
        <DotsSixVertical size={14} weight="bold" />
      </button>
      <span className="w-3 shrink-0 text-[11px] tabular-nums text-muted-foreground">
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
      <Tooltip>
        <TooltipTrigger asChild>
          <Button
            variant="ghost"
            size="icon-xs"
            aria-label="Remove"
            onClick={onRemove}
            // Shown on hover or focus so a resting list reads as just the order.
            className="shrink-0 text-muted-foreground opacity-0 transition-opacity group-focus-within:opacity-100 group-hover:opacity-100 hover:text-foreground"
          >
            <X size={14} />
          </Button>
        </TooltipTrigger>
        <TooltipContent>Remove</TooltipContent>
      </Tooltip>
    </li>
  );
}

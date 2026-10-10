import { cn } from "@/lib/utils";

/** The divider between the two stacked screens — drag it to change the split. */
export function StackDivider({
  onPointerDown,
  dragging,
}: {
  onPointerDown: (e: React.PointerEvent) => void;
  dragging: boolean;
}) {
  return (
    <div
      role="separator"
      aria-orientation="horizontal"
      aria-label="Resize screens"
      onPointerDown={onPointerDown}
      className={cn(
        "group/split relative z-20 h-1.5 shrink-0 cursor-row-resize touch-none",
        "transition-colors",
        dragging ? "bg-brand" : "bg-white/10 hover:bg-white/30",
      )}
    >
      <span
        className={cn(
          "pointer-events-none absolute left-1/2 top-1/2 h-[2px] w-8 -translate-x-1/2 -translate-y-1/2",
          "rounded-full bg-white/50 opacity-0 transition-opacity will-change-[opacity] group-hover/split:opacity-100",
        )}
      />
    </div>
  );
}

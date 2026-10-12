import { cn } from "@/lib/utils";

interface ResizeHandleProps {
  onMouseDown: (e: React.MouseEvent) => void;
  /** Keeps the highlight lit for the whole drag; `:active` is lost once the
   *  pointer leaves the grip. */
  dragging?: boolean;
  /** Accessible name, e.g. "Resize side panel". */
  label?: string;
  className?: string;
  /** `horizontal` splits panes stacked top and bottom. Default `vertical`. */
  orientation?: "vertical" | "horizontal";
  /** Drawn inside the grip (e.g. the side panel's focus marker). */
  children?: React.ReactNode;
}

export function ResizeHandle({
  onMouseDown,
  dragging,
  label,
  className,
  orientation = "vertical",
  children,
}: ResizeHandleProps) {
  const horizontal = orientation === "horizontal";
  return (
    <div
      onMouseDown={onMouseDown}
      role="separator"
      aria-orientation={orientation}
      aria-label={label}
      className={cn(
        horizontal ? "h-1 shrink-0 cursor-row-resize" : "w-1 shrink-0 cursor-col-resize",
        "group relative z-10",
        "hover:bg-brand/40 active:bg-brand/60 transition-colors",
        dragging && "bg-brand/60",
        className,
      )}
    >
      {/* Wider invisible hit area */}
      <div className={horizontal ? "absolute inset-x-0 -top-1 -bottom-1" : "absolute inset-y-0 -left-1 -right-1"} />
      {children}
    </div>
  );
}

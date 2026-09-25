import { cn } from "@/lib/utils";

interface ResizeHandleProps {
  onMouseDown: (e: React.MouseEvent) => void;
  /** Keeps the highlight lit for the whole drag; `:active` is lost once the
   *  pointer leaves the grip. */
  dragging?: boolean;
  /** Accessible name, e.g. "Resize side panel". */
  label?: string;
  className?: string;
  /** Drawn inside the grip (e.g. the split's focus marker). */
  children?: React.ReactNode;
}

export function ResizeHandle({
  onMouseDown,
  dragging,
  label,
  className,
  children,
}: ResizeHandleProps) {
  return (
    <div
      onMouseDown={onMouseDown}
      role="separator"
      aria-orientation="vertical"
      aria-label={label}
      className={cn(
        "w-1 shrink-0 cursor-col-resize group relative z-10",
        "hover:bg-brand/40 active:bg-brand/60 transition-colors",
        dragging && "bg-brand/60",
        className,
      )}
    >
      {/* Wider invisible hit area */}
      <div className="absolute inset-y-0 -left-1 -right-1" />
      {children}
    </div>
  );
}

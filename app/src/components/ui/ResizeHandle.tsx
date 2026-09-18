import { cn } from "@/lib/utils";

interface ResizeHandleProps {
  onMouseDown: (e: React.MouseEvent) => void;
  /**
   * Held down right now. `:active` alone is not enough: the pointer leaves the
   * 1px grip the moment the drag starts, and the highlight has to stay lit for
   * the whole drag, not just the frame the mouse went down on.
   */
  dragging?: boolean;
  /** Accessible name, e.g. "Resize side panel". */
  label?: string;
  className?: string;
  /** Drawn inside the grip, positioned against it. The split's focus marker
   *  lives here rather than inside a pane: a pane showing a browser page is
   *  covered by a native WebView, and anything the DOM draws under that is
   *  simply not on screen. */
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

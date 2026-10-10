import { useRef, type CSSProperties, type ReactNode } from "react";
import { DotsSixVertical, X } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { useScrollFade } from "@/hooks/ui/useScrollFade";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { ViewTabs, type ViewTab } from "@/components/ui/table/ViewTabs";
import { isVertical, type Dock } from "@/stores/lectures/playerPrefsStore";

/** Border on the panel's inner edge — the side that faces the video. */
const INNER_BORDER: Record<Dock, string> = {
  bottom: "border-t",
  top: "border-b",
  left: "border-r",
  right: "border-l",
};

interface MediaDockProps<T extends string> {
  /** The strip's tabs, in the reader's order. */
  tabs: ReadonlyArray<ViewTab<T>>;
  value: T;
  onChange: (tab: T) => void;
  onReorder?: (next: T[]) => void;
  dock: Dock;
  size: number;
  /** Shown or hidden — the panel stays mounted either way and slides. */
  open: boolean;
  /** Mid resize-drag: the size is following a pointer, so it must not ease. */
  resizing: boolean;
  /** Fold the dock away. The control bar fades with the video, so the header
   *  needs its own close. */
  onClose: () => void;
  /** Header press — begins the drag-to-dock gesture. */
  onHeaderPointerDown: (e: React.PointerEvent) => void;
  /** The tab in front. */
  children: ReactNode;
}

/**
 * The player's dock: its box and slide, and the header (drag handle, tab strip,
 * close). What each tab shows is the caller's.
 */
export function MediaDock<T extends string>({
  tabs,
  value,
  onChange,
  onReorder,
  dock,
  size,
  open,
  resizing,
  onClose,
  onHeaderPointerDown,
  children,
}: MediaDockProps<T>) {
  // The outer box animates one dimension to zero while the inner keeps its size,
  // so content is clipped rather than reflowed.
  const outer: CSSProperties = isVertical(dock)
    ? { height: open ? size : 0, minHeight: open ? size : 0, maxHeight: open ? size : 0 }
    : { width: open ? size : 0, minWidth: open ? size : 0, maxWidth: open ? size : 0 };
  const inner: CSSProperties = isVertical(dock)
    ? { height: size, minHeight: size }
    : { width: size, minWidth: size };

  // Not armed by an effect when `open` flips: that runs after the paint that
  // already resized the box, so the panel would snap.
  const sliding = !resizing;

  // The strip scrolls sideways with no visible bar; fades show the overflow.
  // Resizing, re-docking or losing a tab changes what fits; `live` re-hangs it.
  const stripRef = useRef<HTMLDivElement>(null);
  useScrollFade(stripRef, "x", open);

  return (
    <div
      style={outer}
      aria-hidden={!open}
      className={cn(
        "shrink-0 grow-0 overflow-hidden bg-background border-border",
        INNER_BORDER[dock],
        !open && "border-0",
        sliding &&
          (isVertical(dock)
            ? "transition-[height,min-height,max-height] duration-200 ease-out"
            : "transition-[width,min-width,max-width] duration-200 ease-out"),
      )}
    >
      <div style={inner} className="flex h-full flex-col min-h-0 min-w-0">
        {/* The tabs and close button stop pointerdown: the header's dock drag
            captures the pointer, retargeting pointerup so a click never lands. */}
        <div
          onPointerDown={onHeaderPointerDown}
          className="px-2 h-9 flex items-center gap-1.5 border-b border-border shrink-0 cursor-grab active:cursor-grabbing select-none"
        >
          <Tooltip>
            <TooltipTrigger asChild>
              {/* Lifted to the tabs' text, which sits above centre by half the
                  underline's padding. */}
              <span className="mb-2 flex items-center text-muted-foreground hover:text-foreground transition-colors">
                <DotsSixVertical size={12} className="opacity-50" />
              </span>
            </TooltipTrigger>
            <TooltipContent>Drag to dock left, right, top or bottom</TooltipContent>
          </Tooltip>
          {/* No visible scrollbar (classic scrollbars here); the fades carry it. */}
          <div className="min-w-0">
            <div
              ref={stripRef}
              className="min-w-0 overflow-x-auto [scrollbar-width:none] [&::-webkit-scrollbar]:hidden"
              onPointerDown={(e) => e.stopPropagation()}
            >
              <ViewTabs
                tabs={tabs}
                value={value}
                onChange={onChange}
                onReorder={onReorder}
                className="w-max gap-3"
              />
            </div>
          </div>
          <button
            type="button"
            onPointerDown={(e) => e.stopPropagation()}
            onClick={onClose}
            aria-label="Hide panel"
            className="mb-2 ml-auto shrink-0 rounded-full p-1 text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
          >
            <X size={11} weight="bold" />
          </button>
        </div>

        {children}
      </div>
    </div>
  );
}

/** A dock tab with nothing to list yet: centred lines and its one action. */
export function PanelEmpty({ children }: { children: ReactNode }) {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-1.5 px-4 py-6 text-center text-[11px] leading-relaxed">
      {children}
    </div>
  );
}

/** The band the panel would land in, previewed mid-drag in the brand tint. */
export function DockDropPreview({
  dock,
  height,
  width,
}: {
  dock: Dock;
  height: number;
  width: number;
}) {
  const edge: Record<Dock, CSSProperties> = {
    bottom: { left: 0, right: 0, bottom: 0, height },
    top: { left: 0, right: 0, top: 0, height },
    left: { top: 0, bottom: 0, left: 0, width },
    right: { top: 0, bottom: 0, right: 0, width },
  };

  return (
    <div className="absolute inset-0 z-40 pointer-events-none">
      <div
        style={edge[dock]}
        className="absolute rounded-sm bg-brand/25 border border-brand/60 backdrop-blur-[1px] transition-all duration-100"
      />
    </div>
  );
}

/** Divider between the video stack and the panel; drag to resize. */
export function DockResizeHandle({
  dock,
  onPointerDown,
}: {
  dock: Dock;
  onPointerDown: (e: React.PointerEvent) => void;
}) {
  const vertical = isVertical(dock);
  return (
    <div
      role="separator"
      aria-orientation={vertical ? "horizontal" : "vertical"}
      onPointerDown={onPointerDown}
      className={cn(
        "shrink-0 relative z-20 hover:bg-brand/40 active:bg-brand/60 transition-colors",
        vertical ? "h-px w-full cursor-row-resize" : "w-px h-full cursor-col-resize",
      )}
    >
      {/* Wider invisible hit area than the hairline it draws. */}
      <div
        className={cn(
          "absolute",
          vertical ? "inset-x-0 -top-1.5 -bottom-1.5" : "inset-y-0 -left-1.5 -right-1.5",
        )}
      />
    </div>
  );
}

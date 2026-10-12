import { type CSSProperties, type PointerEvent, type ReactNode, type Ref } from "react";
import { X } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { SEPARATOR_W } from "@/components/tabs/strip/constants";

interface StripTabProps {
  title: string;
  icon: ReactNode;
  index: number;
  active: boolean;
  hovered: boolean;
  /** Whether the previous tab is the active or the hovered one. */
  prevActiveOrHovered: boolean;
  /** A tab is being dragged. */
  dragging: boolean;
  grabbed: boolean;
  width: number;
  /** More than one tab, or a browser tab: a closable strip. */
  closable: boolean;
  itemRef: Ref<HTMLDivElement>;
  dragStyle?: CSSProperties;
  onPress: (e: PointerEvent<HTMLDivElement>) => void;
  onClose: () => void;
  onHover: (hovered: boolean) => void;
}

/** One tab in the strip, with the separator column before it. */
export function StripTab({
  title,
  icon,
  index,
  active,
  hovered,
  prevActiveOrHovered,
  dragging,
  grabbed,
  width,
  closable,
  itemRef,
  dragStyle,
  onPress,
  onClose,
  onHover,
}: StripTabProps) {
  // Separator only between two inactive, unhovered neighbours.
  const showSeparator = index > 0 && !active && !hovered && !prevActiveOrHovered && !dragging;
  return (
    <>
      {/* Always SEPARATOR_W wide: the drag maths counts on it. */}
      <span
        className={cn(
          "flex shrink-0 items-center justify-center",
          index === 0 && "hidden",
        )}
        style={{ width: SEPARATOR_W }}
      >
        <span
          className={cn(
            "h-3.5 w-px rounded-full transition-colors",
            showSeparator ? "bg-border" : "bg-transparent",
          )}
        />
      </span>
      <Tooltip>
        <TooltipTrigger asChild>
          <div
            data-tauri-drag-region="false"
            ref={itemRef}
            style={{ flex: "none", width, ...dragStyle }}
            // Keep border geometry stable: selection must not move the contents
            // or resize WebKit's clipped paint area at fractional page zoom.
            className={cn(
              "relative flex h-7 items-center rounded-lg border px-3 overflow-hidden cursor-pointer",
              active
                ? "bg-card text-foreground border-border shadow-xs"
                : cn(
                    "border-transparent text-muted-foreground hover:text-foreground",
                    hovered ? "bg-sidebar-item-hover" : "bg-transparent",
                  ),
              grabbed
                ? "z-10 shadow-md"
                : dragging
                  ? "transition-transform duration-200 ease-out"
                  : "transition-[color,border-color]",
            )}
            onPointerDown={(e) => {
              if (e.button !== 0) return;
              onPress(e);
            }}
            onAuxClick={(e) => {
              if (e.button === 1) onClose();
            }}
            onMouseEnter={() => onHover(true)}
            onMouseLeave={() => onHover(false)}
          >
            {icon && (
              <span className="relative mr-1.5 flex shrink-0 items-center">
                {icon}
              </span>
            )}
            {/* Long titles fade out at the edge instead of an ellipsis. */}
            <span
              style={{
                maskImage:
                  closable
                    ? "linear-gradient(to right, #000 calc(100% - 36px), transparent calc(100% - 14px))"
                    : "linear-gradient(to right, #000 calc(100% - 22px), transparent)",
              }}
              className="relative min-w-0 flex-1 text-[12px] whitespace-nowrap overflow-hidden [text-overflow:clip] py-1"
            >
              {title}
            </span>
            {closable && (
              /* The title mask leaves the × clear without painting another
                 surface over the tab's hover fill. */
              <span
                className={cn(
                  "absolute inset-y-0 right-0 flex items-center pr-1.5 transition-opacity will-change-[opacity]",
                  hovered ? "opacity-100" : "opacity-0",
                )}
              >
                <button
                  onPointerDown={(e) => e.stopPropagation()}
                  onClick={(e) => {
                    e.stopPropagation();
                    onClose();
                  }}
                  aria-label="Close tab"
                  className="rounded-md p-0.5 text-muted-foreground hover:text-foreground hover:bg-sidebar-item-active transition-colors"
                >
                  <X size={11} />
                </button>
              </span>
            )}
          </div>
        </TooltipTrigger>
        <TooltipContent side="bottom">{title}</TooltipContent>
      </Tooltip>
    </>
  );
}

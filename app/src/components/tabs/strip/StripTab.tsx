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
            className={cn(
              "group relative flex h-7 items-center rounded-lg px-3 overflow-hidden cursor-pointer",
              active
                ? "bg-card text-foreground border border-border shadow-xs"
                : "text-muted-foreground hover:text-foreground",
              grabbed
                ? "z-10 shadow-md"
                : dragging
                  ? "transition-transform duration-200 ease-out"
                  : "transition-colors",
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
            {!active && (
              <span
                aria-hidden
                className={cn(
                  "absolute inset-0 rounded-lg transition-colors",
                  hovered && "bg-sidebar-item-hover",
                )}
              />
            )}
            {icon && (
              <span className="relative mr-1.5 flex shrink-0 items-center">
                {icon}
              </span>
            )}
            {/* Long titles fade out at the edge instead of an ellipsis. */}
            <span
              style={{
                maskImage:
                  "linear-gradient(to right, #000 calc(100% - 22px), transparent)",
              }}
              className="relative min-w-0 flex-1 text-[12px] whitespace-nowrap overflow-hidden [text-overflow:clip] py-1"
            >
              {title}
            </span>
            {closable && (
              /* The × overlays the right edge on hover; its gradient
                 stays soft since the title mask already fades the text. */
              <span
                className={cn(
                  "absolute flex items-center pl-6 opacity-0 group-hover:opacity-100 transition-opacity will-change-[opacity]",
                  active
                    ? "inset-y-px right-px pr-1.5 rounded-r-lg bg-gradient-to-l from-card from-40% via-card/70 via-75% to-transparent"
                    : "inset-y-0 right-0 pr-1.5 rounded-r-lg bg-gradient-to-l from-sidebar-item-hover from-40% via-sidebar-item-hover/70 via-75% to-transparent",
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

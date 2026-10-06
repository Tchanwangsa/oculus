import { useState, useSyncExternalStore } from "react";
import { AppWindow, ArrowsOutSimple, Plus, X } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import type { HeaderClaim } from "@/components/tabs/PaneHeader";
import { useTabInfo } from "@/components/tabs/tabInfo";
import { expandSide } from "@/lib/tabRouters";
import { frontOf, type SidePanel } from "@/lib/sideStack";
import { cn } from "@/lib/utils";
import { HOME, useTabStore } from "@/stores/tabStore";

/** How many item icons the switcher stacks before a `+N`. */
const STACKED = 3;

/**
 * The side panel's standalone title row: a switcher over the stack's items,
 * the front item's title, expand and close. A page whose own top row takes the
 * slot (`PaneHeader.tsx`) hides it and draws the same controls. Tooltips keep
 * their default side, upward, clear of a browser item's native page below,
 * which a portal over it hides.
 */
export function SidePanelHeader({
  tabId,
  side,
  claim,
}: {
  tabId: number;
  side: SidePanel;
  claim: HeaderClaim;
}) {
  const tabInfo = useTabInfo();
  const claimed = useSyncExternalStore(claim.subscribe, claim.claimed);
  if (claimed) return null;

  return (
    <div className="flex h-10 shrink-0 items-center gap-0.5 border-b border-border-subtle px-2">
      <Switcher tabId={tabId} side={side} tabInfo={tabInfo} />

      <span className="min-w-0 flex-1 truncate px-1.5 text-[12px] font-medium text-foreground">
        {tabInfo(frontOf(side).path).title}
      </span>

      <PanelActions tabId={tabId} />
    </div>
  );
}

/** The switcher, for a page row holding the header slot. */
export function PanelSwitcher({ tabId, side }: { tabId: number; side: SidePanel }) {
  const tabInfo = useTabInfo();
  return <Switcher tabId={tabId} side={side} tabInfo={tabInfo} />;
}

/** Expand (⌘-click: a new tab) and close; a group that never shrinks. */
export function PanelActions({ tabId }: { tabId: number }) {
  const closeSide = useTabStore((s) => s.closeSide);
  return (
    <div className="flex shrink-0 items-center gap-0.5">
      <Tooltip>
        <TooltipTrigger asChild>
          <Button
            variant="ghost"
            size="icon-xs"
            onClick={(e) => expandSide(tabId, e.metaKey || e.ctrlKey)}
            aria-label="Open as full page"
            className="text-muted-foreground hover:text-foreground"
          >
            <ArrowsOutSimple size={14} />
          </Button>
        </TooltipTrigger>
        <TooltipContent className="flex flex-col items-start gap-0.5">
          Open as full page
          <span className="text-[11px] text-background/60">⌘-click for a new tab</span>
        </TooltipContent>
      </Tooltip>

      <Tooltip>
        <TooltipTrigger asChild>
          <Button
            variant="ghost"
            size="icon-xs"
            onClick={() => closeSide(tabId)}
            aria-label="Close side panel"
            className="text-muted-foreground hover:text-foreground"
          >
            <X size={14} />
          </Button>
        </TooltipTrigger>
        <TooltipContent>Close side panel</TooltipContent>
      </Tooltip>
    </div>
  );
}

/**
 * The items' icons, front first and then in list order; a click lists them
 * all, to bring one forward or remove it.
 */
function Switcher({
  tabId,
  side,
  tabInfo,
}: {
  tabId: number;
  side: SidePanel;
  tabInfo: ReturnType<typeof useTabInfo>;
}) {
  const frontSide = useTabStore((s) => s.frontSide);
  const removeSide = useTabStore((s) => s.removeSide);
  const pushSide = useTabStore((s) => s.pushSide);
  const [open, setOpen] = useState(false);

  const front = frontOf(side);
  const stacked = [front, ...side.items.filter((i) => i !== front)].slice(0, STACKED);
  const more = side.items.length - stacked.length;

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <button
          type="button"
          aria-label="Side panel items"
          className="flex h-6 shrink-0 items-center rounded-full px-1 text-muted-foreground transition-colors hover:text-foreground data-[state=open]:text-foreground"
        >
          {stacked.map((item, i) => (
            // Each chip is a bordered disc, ringed in the header's own
            // background for a gap, so the overlap reads as a stack; the
            // front one draws on top.
            <span
              key={item.id}
              className={cn(
                "relative flex size-5 items-center justify-center rounded-full border border-border bg-card ring-2 ring-card",
                i > 0 && "-ml-1.5",
              )}
              style={{ zIndex: stacked.length - i }}
            >
              {tabInfo(item.path, 12).icon ?? <AppWindow size={12} />}
            </span>
          ))}
          {more > 0 && (
            <span className="ml-1 text-[11px] tabular-nums">+{more}</span>
          )}
        </button>
      </PopoverTrigger>
      <PopoverContent
        align="start"
        className="w-64 p-1"
        // Focus the list, not its first row, which would open ringed; Tab
        // still walks the rows, and the ring shows there.
        onOpenAutoFocus={(e) => {
          e.preventDefault();
          (e.target as HTMLElement).focus({ preventScroll: true });
        }}
      >
        {side.items.map((item) => {
          const { title, icon } = tabInfo(item.path);
          const isFront = item.id === front.id;
          return (
            // The row and its × are siblings: WebKit drops a click on a
            // button nested in a button.
            <div
              key={item.id}
              className={cn(
                "flex items-center rounded-md pr-1 transition-colors",
                isFront
                  ? "bg-accent text-foreground"
                  : "text-muted-foreground hover:bg-accent hover:text-foreground",
              )}
            >
              <button
                type="button"
                onClick={() => {
                  frontSide(tabId, item.id);
                  setOpen(false);
                }}
                aria-current={isFront || undefined}
                className="flex min-w-0 flex-1 items-center gap-2 px-2 py-1.5 text-left text-[12.5px]"
              >
                <span className="flex size-3.5 shrink-0 items-center justify-center">
                  {icon ?? <AppWindow size={13} />}
                </span>
                <span className="min-w-0 flex-1 truncate">{title}</span>
              </button>
              <Button
                variant="ghost"
                size="icon-xs"
                onClick={() => removeSide(tabId, item.id)}
                aria-label={`Close ${title}`}
                className="text-muted-foreground hover:text-foreground"
              >
                <X size={11} />
              </Button>
            </div>
          );
        })}
        <button
          type="button"
          onClick={() => {
            pushSide(tabId, HOME);
            setOpen(false);
          }}
          className="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-[12.5px] text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
        >
          <span className="flex size-3.5 shrink-0 items-center justify-center">
            <Plus size={13} />
          </span>
          New side tab
        </button>
      </PopoverContent>
    </Popover>
  );
}

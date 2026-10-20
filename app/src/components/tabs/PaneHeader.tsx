import {
  createContext,
  useContext,
  useLayoutEffect,
  useRef,
  type ReactNode,
} from "react";
import { PanelActions, PanelSwitcher } from "@/components/tabs/SidePanelHeader";
import { useScrollFade } from "@/hooks/ui/useScrollFade";
import type { SidePanel } from "@/lib/shell/sideStack";
import { cn } from "@/lib/utils";

/**
 * A page's top row doubling as the side panel's header. `TabPane` gives a
 * side pane a slot; a page row built on `PaneHeaderRow` takes it, drawing the
 * panel's switcher at its start and expand and × at its end, and
 * `SidePanelHeader` hides while it is taken. The claim lands in a layout
 * effect, so the standalone header shows until the row mounts (a lazy route's
 * `LoadingFill`) and is gone before the row first paints. In the main pane
 * the row renders as a plain row.
 */

/** Which page row holds the side panel's header, if any. */
export interface HeaderClaim {
  subscribe: (listener: () => void) => () => void;
  claimed: () => boolean;
  /** The holding row's height, for the find bar floating below it. */
  height: () => number;
  take: (row: HTMLElement) => () => void;
}

export function createHeaderClaim(): HeaderClaim {
  let row: HTMLElement | null = null;
  let height = 0;
  const listeners = new Set<() => void>();
  const emit = () => listeners.forEach((l) => l());
  return {
    subscribe: (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    claimed: () => row != null,
    height: () => height,
    take: (el) => {
      row = el;
      height = el.offsetHeight;
      emit();
      return () => {
        if (row !== el) return;
        row = null;
        height = 0;
        emit();
      };
    },
  };
}

interface SideSlot {
  tabId: number;
  side: SidePanel;
  claim: HeaderClaim;
}

const SideSlotContext = createContext<SideSlot | null>(null);

export const SideSlotProvider = SideSlotContext.Provider;

/** Whether this page renders as the side panel's front item. */
export function useInSidePanel(): boolean {
  return useContext(SideSlotContext) != null;
}

/**
 * A page's top row. In the side panel it takes the header slot and adds the
 * panel's controls, which never shrink; `standalone` leaves the slot to
 * `SidePanelHeader` (a page whose row has no trail, like chat history).
 */
export function PaneHeaderRow({
  className,
  standalone,
  children,
}: {
  className: string;
  standalone?: boolean;
  children: ReactNode;
}) {
  const ctx = useContext(SideSlotContext);
  const slot = standalone ? null : ctx;
  const claim = slot?.claim;
  const ref = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    const row = ref.current;
    return claim && row ? claim.take(row) : undefined;
  }, [claim]);

  return (
    <div ref={ref} className={cn(className, slot && "px-2")}>
      {slot && <PanelSwitcher tabId={slot.tabId} side={slot.side} />}
      {children}
      {slot && <PanelActions tabId={slot.tabId} />}
    </div>
  );
}

/** The common 48px top bar for Chat, Studio and planning pages. */
export function PageHeader({ children, className }: { children: ReactNode; className?: string }) {
  return <PaneHeaderRow className={cn("flex h-12 shrink-0 items-center gap-2.5 border-b border-border-subtle px-6", className)}>{children}</PaneHeaderRow>;
}

/** Keeps a side panel's trail scrollable without changing its parent path. */
export function PaneTrail({ children }: { children: ReactNode }) {
  return useInSidePanel() ? <SideTrail>{children}</SideTrail> : <>{children}</>;
}

function SideTrail({ children }: { children: ReactNode }) {
  const ref = useRef<HTMLDivElement>(null);
  useScrollFade(ref, "x");
  return (
    <div ref={ref} className="-mx-1 min-w-0 flex-1 overflow-x-auto [scrollbar-width:none] [&::-webkit-scrollbar]:hidden">
      <div className="flex w-max items-center gap-2.5 p-1">{children}</div>
    </div>
  );
}

/**
 * A page row's title: truncated in the main pane; in the side panel's
 * scroller it keeps its full width and scrolls instead.
 */
export function PaneTitle({ className, children }: { className?: string; children: ReactNode }) {
  const inSide = useInSidePanel();
  return (
    <h1
      className={cn(
        inSide ? "shrink-0 whitespace-nowrap" : "min-w-0 flex-1 truncate",
        "text-[13px] font-semibold text-foreground",
        className,
      )}
    >
      {children}
    </h1>
  );
}

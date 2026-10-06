import {
  memo,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
  type ReactNode,
} from "react";
import { createMemoryRouter, parsePath, RouterProvider } from "react-router-dom";
import { routes } from "@/routes";
import { TabContext } from "@/components/tabs/TabContext";
import { SidePanelHeader } from "@/components/tabs/SidePanelHeader";
import {
  createHeaderClaim,
  SideSlotProvider,
  type HeaderClaim,
} from "@/components/tabs/PaneHeader";
import { ResizeHandle } from "@/components/ui/ResizeHandle";
import { PageFind } from "@/components/ui/PageFind";
import { routerFor } from "@/lib/tabRouters";
import { clampRatio, SIDE_RATIO, type SideItem } from "@/lib/sideStack";
import { cancelRecentTab, recordRecentTab } from "@/stores/recentTabsStore";
import {
  sideFront,
  useTabStore,
  type AppTab,
  type PaneSide,
  type PaneState,
} from "@/stores/tabStore";
import { cn } from "@/lib/utils";

/**
 * One tab's page, and its side panel's front item beside it when open. Each
 * pane owns a memory router; the main pane stays mounted, so switching tabs
 * keeps scroll, drafts and component state.
 */

const TabPane = memo(function TabPane({
  tab,
  active,
}: {
  tab: AppTab;
  active: boolean;
}) {
  const tabId = tab.id;
  const front = sideFront(tab);
  const focusPane = useTabStore((s) => s.focusPane);
  const setSideRatio = useTabStore((s) => s.setSideRatio);
  // The ratio on screen mid-drag; the store hears it once, on release.
  const [dragRatio, setDragRatio] = useState<number | null>(null);
  const [dragging, setDragging] = useState(false);
  const stackRef = useRef<HTMLDivElement>(null);
  // Which page row, if any, stands in for the side panel's header.
  const [claim] = useState(createHeaderClaim);
  const sidePanel = tab.side;
  const slot = useMemo(
    () => (sidePanel ? { tabId, side: sidePanel, claim } : null),
    [tabId, sidePanel, claim],
  );

  // A fraction, not a width, so the divider holds its place when the stack resizes.
  const onHandleDown = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    setDragging(true);
    document.body.style.cursor = "col-resize";
    document.body.style.webkitUserSelect = "none";
    let latest: number | null = null;
    const onMove = (ev: MouseEvent) => {
      const box = stackRef.current?.getBoundingClientRect();
      if (!box || box.width === 0) return;
      latest = clampRatio((ev.clientX - box.left) / box.width);
      setDragRatio(latest);
    };
    const end = () => {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", end);
      document.body.style.cursor = "";
      document.body.style.webkitUserSelect = "";
      if (latest != null) setSideRatio(tabId, latest);
      setDragRatio(null);
      setDragging(false);
    };
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", end);
  }, [tabId, setSideRatio]);

  const ratio = dragRatio ?? tab.side?.ratio ?? SIDE_RATIO;

  return (
    /* Hidden with `visibility`, never `display: none`: display-none zeroes
       every rect, resets scroll and fires ResizeObservers at width 0, losing
       the state panes exist to keep. Stacks are `absolute inset-0` so nothing
       reflows when one comes forward. */
    <div
      ref={stackRef}
      className="absolute inset-0 flex"
      style={{ visibility: active ? "visible" : "hidden" }}
      inert={!active}
    >
      <Pane
        pane={tab}
        tabId={tabId}
        side="main"
        tabActive={active}
        onFocus={focusPane}
        style={front ? { flex: `0 0 ${ratio * 100}%` } : undefined}
      />
      {tab.side && front && (
        <>
          {/* The divider carries the focus marker (indigo edge on the focused
              side): inside a pane, a native browser WebView would cover it. */}
          <ResizeHandle
            onMouseDown={onHandleDown}
            dragging={dragging}
            label="Resize side panel"
            className="bg-border"
          >
            <span
              aria-hidden
              className={cn(
                "pointer-events-none absolute inset-y-0 w-px bg-brand",
                tab.focus === "main" ? "left-0" : "right-0",
              )}
            />
          </ResizeHandle>
          {/* Only the front item is mounted; the rest are a path and a
              router (`lib/tabRouters.ts`). Focus is taken on the column, so
              a click on the header focuses the side panel too. */}
          <div
            onPointerDownCapture={() => focusPane(tabId, "side")}
            onFocusCapture={() => focusPane(tabId, "side")}
            className="flex min-w-0 flex-1 flex-col"
          >
            <SidePanelHeader tabId={tabId} side={tab.side} claim={claim} />
            <SideSlotProvider value={slot}>
              <Pane
                key={front.id}
                pane={front}
                tabId={tabId}
                side="side"
                tabActive={active}
                claim={claim}
              />
            </SideSlotProvider>
          </div>
        </>
      )}
    </div>
  );
});

export default TabPane;

/** A pane's first router entry: its path, plus any state the pane was
 *  opened with (`SideItem.entryState`, `AppTab.entryState`). */
function firstEntry(pane: PaneState & { entryState?: unknown }) {
  const state = pane.entryState;
  return state === undefined ? pane.path : { ...parsePath(pane.path), state };
}

/**
 * One pane and its page. The router outlives the mount (`routerFor`), so a
 * side item brought back to the front keeps its history. The pane id keys the
 * router registry, lecture playback and Recent trail.
 */
function Pane({
  pane,
  tabId,
  side,
  tabActive,
  onFocus,
  style,
  claim,
}: {
  pane: PaneState | SideItem;
  tabId: number;
  side: PaneSide;
  tabActive: boolean;
  /** Set focus on press or focus within; the side panel's column does it
   *  for its pane instead. */
  onFocus?: (tabId: number, side: PaneSide) => void;
  style?: React.CSSProperties;
  /** The side panel's header slot, which the find bar floats below. */
  claim?: HeaderClaim;
}) {
  const id = pane.id;
  const rootRef = useRef<HTMLDivElement>(null);
  // Fetched once per mount, never re-created (that would unmount the page);
  // the pane's path afterwards is an output of this router, not an input.
  const [router] = useState(() =>
    routerFor(id, () =>
      createMemoryRouter(routes, { initialEntries: [firstEntry(pane)] }),
    ),
  );
  const [activeAtMount] = useState(tabActive);

  useEffect(() => {
    // `subscribe` doesn't report the page on show at mount, so it's recorded
    // here — only in the front tab: a reload restores every tab at once.
    const { pathname, search } = router.state.location;
    if (activeAtMount) recordRecentTab(id, pathname + search);
    // A side item sent to the back within the dwell was only passed through.
    return () => cancelRecentTab(id);
  }, [id, router, activeAtMount]);

  const context = useMemo(
    () => ({ id, tabId, side, active: tabActive }),
    [id, tabId, side, tabActive],
  );

  return (
    <div
      ref={rootRef}
      // Capture phase, so focus moves before the clicked control reacts.
      onPointerDownCapture={onFocus && (() => onFocus(tabId, side))}
      onFocusCapture={onFocus && (() => onFocus(tabId, side))}
      // The page's text selects; the side panel header and shell don't.
      data-select-scope
      className="relative min-h-0 min-w-0 flex-1"
      style={style}
    >
      <TabContext.Provider value={context}>
        <RouterProvider router={router} />
      </TabContext.Provider>
      {/* ⌘F's fallback for whatever page the pane shows (`lib/find.ts`). */}
      {claim ? (
        <BelowHeader claim={claim}>
          <PageFind rootRef={rootRef} page={id} />
        </BelowHeader>
      ) : (
        <PageFind rootRef={rootRef} page={id} />
      )}
    </div>
  );
}

/**
 * Moves the floating find bar below a page row holding the side panel's
 * header, clear of expand and ×. Zero height, so it takes no clicks itself.
 */
function BelowHeader({ claim, children }: { claim: HeaderClaim; children: ReactNode }) {
  const top = useSyncExternalStore(claim.subscribe, claim.height);
  return (
    <div className="absolute inset-x-0 h-0" style={{ top }}>
      {children}
    </div>
  );
}

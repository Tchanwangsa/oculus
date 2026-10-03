import { memo, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { createMemoryRouter, RouterProvider } from "react-router-dom";
import { routes } from "@/routes";
import { TabContext } from "@/components/tabs/TabContext";
import { ResizeHandle } from "@/components/ui/ResizeHandle";
import { registerTabRouter, unregisterTabRouter } from "@/lib/tabRouters";
import { cancelRecentTab, recordRecentTab } from "@/stores/recentTabsStore";
import { useTabStore, type AppTab, type PaneSide, type PaneState } from "@/stores/tabStore";
import { cn } from "@/lib/utils";

/**
 * One tab's page, or its two pages side by side when split. Each pane owns a
 * memory router and stays mounted, so switching tabs keeps scroll, drafts and
 * component state.
 */

/** The main half's share of the stack, shared by every tab. */
const RATIO_KEY = "oculus-split-ratio";
const RATIO_DEFAULT = 0.5;
const RATIO_MIN = 0.25;
const RATIO_MAX = 0.75;

function storedRatio(): number {
  try {
    const r = Number(localStorage.getItem(RATIO_KEY));
    if (Number.isFinite(r) && r >= RATIO_MIN && r <= RATIO_MAX) return r;
  } catch {
    /* private mode — the default is fine */
  }
  return RATIO_DEFAULT;
}

const TabPane = memo(function TabPane({
  tab,
  active,
}: {
  tab: AppTab;
  active: boolean;
}) {
  const split = tab.split;
  const focusPane = useTabStore((s) => s.focusPane);
  const [ratio, setRatio] = useState(storedRatio);
  const [dragging, setDragging] = useState(false);
  const stackRef = useRef<HTMLDivElement>(null);

  // A fraction, not a width, so the divider holds its place when the stack resizes.
  const onHandleDown = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    setDragging(true);
    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";
    const onMove = (ev: MouseEvent) => {
      const box = stackRef.current?.getBoundingClientRect();
      if (!box || box.width === 0) return;
      const next = (ev.clientX - box.left) / box.width;
      setRatio(Math.min(RATIO_MAX, Math.max(RATIO_MIN, next)));
    };
    const end = () => {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", end);
      document.body.style.cursor = "";
      document.body.style.userSelect = "";
      setDragging(false);
    };
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", end);
  }, []);

  useEffect(() => {
    if (dragging) return;
    try {
      localStorage.setItem(RATIO_KEY, String(ratio));
    } catch {
      /* quota — the ratio is expendable */
    }
  }, [ratio, dragging]);

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
        tabId={tab.id}
        side="main"
        tabActive={active}
        onFocus={focusPane}
        style={split ? { flex: `0 0 ${ratio * 100}%` } : undefined}
      />
      {split && (
        <>
          {/* The divider carries the focus marker (indigo edge on the focused
              side): inside a pane, a native browser WebView would cover it. */}
          <ResizeHandle
            onMouseDown={onHandleDown}
            dragging={dragging}
            label="Resize split"
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
          <Pane
            key={split.id}
            pane={split}
            tabId={tab.id}
            side="split"
            tabActive={active}
            onFocus={focusPane}
          />
        </>
      )}
    </div>
  );
});

export default TabPane;

/**
 * One pane and its router, mounted for as long as the pane exists. The pane id
 * keys the router registry, side panel, lecture playback and Recent trail.
 */
function Pane({
  pane,
  tabId,
  side,
  tabActive,
  onFocus,
  style,
}: {
  pane: PaneState;
  tabId: number;
  side: PaneSide;
  tabActive: boolean;
  onFocus: (tabId: number, side: PaneSide) => void;
  style?: React.CSSProperties;
}) {
  const id = pane.id;
  // Built once and never re-created (that would unmount the page); the pane's
  // path afterwards is an output of this router, not an input.
  const [router] = useState(() =>
    createMemoryRouter(routes, { initialEntries: [pane.path] }),
  );
  // `subscribe` doesn't fire for the first location, so it's recorded separately.
  const [seed] = useState(() => ({ path: pane.path, active: tabActive }));

  /** This pane's history (location keys + cursor), since a memory router has
   *  no `window.history` index; yields `canBack`/`canForward` for `tabStore`. */
  const stack = useRef<string[]>([router.state.location.key]);
  const at = useRef(0);

  useEffect(() => {
    registerTabRouter(id, router);
    let seen = router.state.location.key;
    const unsubscribe = router.subscribe((state) => {
      // `subscribe` also fires for navigation state; only a new key is a move.
      const key = state.location.key;
      if (key === seen) return;
      seen = key;
      if (state.historyAction === "PUSH") {
        stack.current = [...stack.current.slice(0, at.current + 1), key];
        at.current = stack.current.length - 1;
      } else if (state.historyAction === "POP") {
        const i = stack.current.indexOf(key);
        if (i !== -1) at.current = i;
      } else {
        stack.current[at.current] = key;
      }
      const path = state.location.pathname + state.location.search;
      useTabStore.getState().setPath(id, path, {
        canBack: at.current > 0,
        canForward: at.current < stack.current.length - 1,
      });
      // Keyed by pane id so a page this pane only passed through is dropped.
      recordRecentTab(id, path);
    });
    return () => {
      unsubscribe();
      cancelRecentTab(id);
      unregisterTabRouter(id);
    };
  }, [id, router]);

  useEffect(() => {
    // Only the front pane: a reload restores every tab at once.
    if (seed.active) recordRecentTab(id, seed.path);
  }, [id, seed]);

  const context = useMemo(
    () => ({ id, tabId, side, active: tabActive }),
    [id, tabId, side, tabActive],
  );

  return (
    <div
      // Capture phase, so focus moves before the clicked control reacts.
      onPointerDownCapture={() => onFocus(tabId, side)}
      onFocusCapture={() => onFocus(tabId, side)}
      className="relative min-w-0 flex-1"
      style={style}
    >
      <TabContext.Provider value={context}>
        <RouterProvider router={router} />
      </TabContext.Provider>
    </div>
  );
}

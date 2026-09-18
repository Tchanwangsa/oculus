import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { createMemoryRouter, RouterProvider } from "react-router-dom";
import { routes } from "@/routes";
import { TabContext } from "@/components/tabs/TabContext";
import { ResizeHandle } from "@/components/ui/ResizeHandle";
import { registerTabRouter, unregisterTabRouter } from "@/lib/tabRouters";
import { cancelRecentTab, recordRecentTab } from "@/stores/recentTabsStore";
import { useTabStore, type AppTab, type PaneSide, type PaneState } from "@/stores/tabStore";
import { cn } from "@/lib/utils";

/**
 * One tab's page — or, when the tab is split, its two pages side by side.
 *
 * A tab used to be a stored path that the one router was told to go to, which
 * meant switching tabs unmounted the page you left — scroll position, typed
 * drafts, expanded rows and every bit of component state with it. Here each
 * *pane* gets a memory router of its own and stays mounted; switching tabs
 * only changes which pane stack is showing, and splitting a tab adds a second
 * router beside the first rather than moving anything.
 */

/** How wide the main half is, as a fraction of the pane stack. Shared by every
 *  tab, the way the side panel's width is: what you opened is per tab, how
 *  big it is is the window's furniture. */
const RATIO_KEY = "oculus-split-ratio";
const RATIO_DEFAULT = 0.5;
/** Neither half may be squeezed past this — a pane much narrower than a
 *  sidebar is a column of wrapped words, not a page. */
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

export default function TabPane({
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

  // Dragged as a fraction rather than a width, so the divider keeps its place
  // when the window, the sidebar or the side panel changes what the stack has
  // to divide up.
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
    /* Hidden with `visibility`, never `display: none`. Display-none drops the
       subtree out of layout: every rect goes to zero, scroll containers reset,
       and the ResizeObservers in the PDF viewer, the thread map, the chat's
       stick-to-bottom and the transcript virtualiser all fire at width 0 — so
       coming back to a tab would mean rebuilding the very state panes exist to
       keep. `visibility` leaves layout, rects and scroll positions exactly as
       they were and costs only paint. The stacks are `absolute inset-0` at
       identical size for the same reason: nothing reflows when one comes
       forward. */
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
          {/* A hairline the drag lives on, and the split's focus marker: an
              indigo edge on whichever side the shell is driving.

              The marker cannot live *inside* a pane. A pane showing a browser
              page is covered by a native WebView that the DOM cannot draw
              over, so a ring or an inner border would vanish for exactly the
              half a split is most often used for. The seam is outside both
              panes and always visible. */}
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
}

/**
 * One pane: its router, mounted for as long as the pane exists.
 *
 * Its id is what everything below a tab is keyed by — the router registry, the
 * side panel's entry, a playing lecture, the Recent trail — so a split half is
 * a first-class pane and not a special case of the tab around it.
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
  // Built once, from the path the pane was opened (or restored) at. Re-creating
  // it on a re-render would be exactly the unmount this arrangement exists to
  // avoid, so it is never re-created — the pane's path afterwards is an output
  // of this router, not an input to it.
  const [router] = useState(() =>
    createMemoryRouter(routes, { initialEntries: [pane.path] }),
  );
  // A pane's seed only reaches the trail from here: `subscribe` fires on
  // changes, and a pane's first location is not one.
  const [seed] = useState(() => ({ path: pane.path, active: tabActive }));

  /**
   * This pane's own history: the stack of location keys, and where in it we
   * are. The strip's arrows used to read React Router's index off
   * `window.history.state`, which a memory router has none of — so the same
   * bookkeeping happens here, per pane. A push truncates whatever was ahead, a
   * pop moves the cursor to the key it landed on, and a replace swaps the key
   * in place; `tabStore` gets the two booleans that fall out of it.
   */
  const stack = useRef<string[]>([router.state.location.key]);
  const at = useRef(0);

  useEffect(() => {
    registerTabRouter(id, router);
    let seen = router.state.location.key;
    const unsubscribe = router.subscribe((state) => {
      // `subscribe` also fires for navigation state; a key is unique to an
      // entry, so a new one is the only thing that means we moved.
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
      // The sidebar's Recent trail is the router's, and every router is a
      // pane now — so each one records its own moves. Handing it this pane's
      // id is what lets it drop a page we only passed through: the next move
      // here cancels the one before it, while another pane's moves are its
      // own.
      recordRecentTab(id, path);
    });
    return () => {
      unsubscribe();
      cancelRecentTab(id);
      unregisterTabRouter(id);
    };
  }, [id, router]);

  useEffect(() => {
    // Only the pane that mounts in front: a reload brings the whole strip back
    // at once, and the trail must not be rewritten as the strip's order.
    if (seed.active) recordRecentTab(id, seed.path);
  }, [id, seed]);

  const context = useMemo(
    () => ({ id, tabId, side, active: tabActive }),
    [id, tabId, side, tabActive],
  );

  return (
    <div
      // Capture, so the focus lands before whatever was clicked reacts to the
      // click — a ⌘K result opened from a row in this pane must already be
      // navigating *this* half. `focus` as well as pointer, for the keyboard.
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

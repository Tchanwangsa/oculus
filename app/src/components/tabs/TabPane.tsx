import { useEffect, useMemo, useRef, useState } from "react";
import { createMemoryRouter, RouterProvider } from "react-router-dom";
import { routes } from "@/routes";
import { TabContext } from "@/components/tabs/TabContext";
import { registerTabRouter, unregisterTabRouter } from "@/lib/tabRouters";
import { recordRecentTab } from "@/stores/recentTabsStore";
import { useTabStore, type AppTab } from "@/stores/tabStore";

/**
 * One tab's page, mounted for as long as the tab exists.
 *
 * A tab used to be a stored path that the one router was told to go to, which
 * meant switching tabs unmounted the page you left — scroll position, typed
 * drafts, expanded rows and every bit of component state with it. Here each
 * tab gets a memory router of its own and stays mounted; switching tabs only
 * changes which pane is showing.
 */
export default function TabPane({
  tab,
  active,
}: {
  tab: AppTab;
  active: boolean;
}) {
  const id = tab.id;
  // Built once, from the path the tab was opened (or restored) at. Re-creating
  // it on a re-render would be exactly the unmount this arrangement exists to
  // avoid, so it is never re-created — the tab's path afterwards is an output
  // of this router, not an input to it.
  const [router] = useState(() =>
    createMemoryRouter(routes, { initialEntries: [tab.path] }),
  );
  // A pane's seed only reaches the trail from here: `subscribe` fires on
  // changes, and a tab's first location is not one.
  const [seed] = useState(() => ({ path: tab.path, active }));

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
      // pane now — so each one records its own moves.
      recordRecentTab(path);
    });
    return () => {
      unsubscribe();
      unregisterTabRouter(id);
    };
  }, [id, router]);

  useEffect(() => {
    // Only the pane that mounts in front: a reload brings the whole strip back
    // at once, and the trail must not be rewritten as the strip's order.
    if (seed.active) recordRecentTab(seed.path);
  }, [seed]);

  const context = useMemo(() => ({ id, active }), [id, active]);

  return (
    /* Hidden with `visibility`, never `display: none`. Display-none drops the
       subtree out of layout: every rect goes to zero, scroll containers reset,
       and the ResizeObservers in the PDF viewer, the thread map, the chat's
       stick-to-bottom and the transcript virtualiser all fire at width 0 — so
       coming back to a tab would mean rebuilding the very state panes exist to
       keep. `visibility` leaves layout, rects and scroll positions exactly as
       they were and costs only paint. The panes are stacked `absolute inset-0`
       at identical size for the same reason: nothing reflows when one comes
       forward. */
    <div
      className="absolute inset-0"
      style={{ visibility: active ? "visible" : "hidden" }}
      inert={!active}
    >
      <TabContext.Provider value={context}>
        <RouterProvider router={router} />
      </TabContext.Provider>
    </div>
  );
}

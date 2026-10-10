import { useEffect } from "react";
import { matchRoutes } from "react-router-dom";
import { routes } from "@/routes";
import { useTabStore } from "@/stores/shell/tabStore";

/**
 * ⌘-click (or middle click) opens a new tab, app-wide, from one capture-phase
 * listener — the in-app twin of `app/src/layouts/AppLayout.tsx`'s external
 * link net. react-router leaves a modified click to the browser, which in a
 * Tauri webview reloads the whole app.
 *
 * The nearest carrier up the tree says where: an `href` (every `Link`), a
 * `data-tab-href` on button rows that must keep `navigateActive`'s plain-click
 * rules or open beside the page, or `data-tab-skip` on a nested control
 * that keeps its own click.
 */
const CARRIER = "a[href], [data-tab-href], [data-tab-skip]";

/** The in-app route the click leads to, or null. Matched against the route
 *  table: an agent's absolute file links also start with `/`
 *  (`lib/files/openFile.ts`). */
function tabTarget(from: EventTarget | null): string | null {
  const el = from instanceof Element ? from.closest(CARRIER) : null;
  if (!el || el.hasAttribute("data-tab-skip")) return null;
  const path = el.getAttribute("data-tab-href") ?? el.getAttribute("href");
  if (!path || !path.startsWith("/")) return null;
  return matchRoutes(routes, path.split(/[?#]/)[0]) ? path : null;
}

export function useNewTabClicks(): void {
  useEffect(() => {
    const onClick = (e: MouseEvent) => {
      if (e.defaultPrevented) return;
      // A middle click arrives as `auxclick`.
      const wants = e.button === 1 || (e.button === 0 && (e.metaKey || e.ctrlKey));
      if (!wants) return;
      const path = tabTarget(e.target);
      if (!path) return;
      // Capture phase: stop the anchor, `Link` and row handlers still ahead.
      e.preventDefault();
      e.stopImmediatePropagation();
      useTabStore.getState().addTab(path);
    };
    document.addEventListener("click", onClick, true);
    document.addEventListener("auxclick", onClick, true);
    return () => {
      document.removeEventListener("click", onClick, true);
      document.removeEventListener("auxclick", onClick, true);
    };
  }, []);
}

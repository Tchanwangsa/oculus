import { useEffect } from "react";
import { matchRoutes } from "react-router-dom";
import { routes } from "@/routes";
import { useTabStore } from "@/stores/tabStore";

/**
 * ⌘-click opens a new tab, everywhere, without the thing you clicked having to
 * know.
 *
 * This is the in-app twin of the external-link net in
 * `app/src/layouts/AppLayout.tsx`: one capture-phase listener on `document`,
 * so a row, a crumb, a card title or a sidebar entry gets the web's own rule
 * for "same thing, somewhere else" for free. It had been per-call-site — a
 * shared `openFromClick` in the crumb rows, a hand-written `e.metaKey` in
 * three more — which is why most of the app quietly did nothing on ⌘-click,
 * and why a `Link` did something worse: react-router deliberately leaves a
 * *modified* click to the browser, and the browser's answer inside a Tauri
 * webview is to reload the whole app out from under the strip.
 *
 * Two ways to say where something leads, in priority order up the tree:
 *
 * - **`href`** — every `Link` and `NavLink` already has one, resolved to an
 *   absolute path by the pane's router, so they are all covered with no edit.
 * - **`data-tab-href`** — for the rows that are buttons rather than anchors,
 *   which in this app is most of the sidebar, the crumb rows — they stay
 *   buttons so that a plain click goes through `navigateActive`'s departure
 *   rules — and every list row that opens a file or a lecture in the side
 *   panel. A peek has no href of its own; the attribute names the full page
 *   that *is* its tab-sized form. It is left off where there is no such page:
 *   a binary handed to the system viewer, or a chat thread, which is selected
 *   in the one chat page rather than routed to.
 *
 * And one way to opt out: **`data-tab-skip`** on a control nested inside a row
 * that leads somewhere — a download button inside a lecture row, say. Found
 * first on the way up, it stops the walk, so the control keeps its own click.
 */
const CARRIER = "a[href], [data-tab-href], [data-tab-skip]";

/**
 * The in-app route the click leads to, or null.
 *
 * Matched against the real route table rather than sniffed for a leading
 * slash, because an absolute path is not proof of anything here: a CLI agent's
 * markdown is full of `/…/com.tchan.oculus/courses/…` links, which are files
 * and are handled elsewhere (`lib/openFile.ts`). A path no route claims is not
 * ours.
 */
function tabTarget(from: EventTarget | null): string | null {
  const el = from instanceof Element ? from.closest(CARRIER) : null;
  if (!el || el.hasAttribute("data-tab-skip")) return null;
  const path = el.getAttribute("data-tab-href") ?? el.getAttribute("href");
  if (!path || !path.startsWith("/")) return null;
  return matchRoutes(routes, path.split(/[?#]/)[0]) ? path : null;
}

/** Installs the net. Called once, from the shell. */
export function useNewTabClicks(): void {
  useEffect(() => {
    const onClick = (e: MouseEvent) => {
      if (e.defaultPrevented) return;
      // ⌘/Ctrl-click, or a middle click — which arrives as `auxclick`.
      const wants = e.button === 1 || (e.button === 0 && (e.metaKey || e.ctrlKey));
      if (!wants) return;
      const path = tabTarget(e.target);
      if (!path) return;
      // Capture phase, so this is the *only* handler that runs: an anchor's
      // default navigation, a `Link`'s own onClick and the row's plain-click
      // handler are all still ahead of us, and every one of them would take
      // the click somewhere that is not a new tab.
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

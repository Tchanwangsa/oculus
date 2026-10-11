import { useCallback, useRef, useState, type RefObject } from "react";
import { browser, type BrowserTab, type FindResult } from "@/lib/browser";
import { useTauriEvent } from "@/hooks/backend/useEvents";
import { useFindTarget } from "@/lib/menu/find";

/** Find in page: WebKit's find via Rust, wired to the menu's ⌘F / ⌘G. */
export function useBrowserFind(
  tab: BrowserTab | undefined,
  id: number,
  paneId: number | undefined,
  rootRef: RefObject<HTMLDivElement | null>,
) {
  const findRef = useRef<HTMLInputElement>(null);
  const [find, setFind] = useState<{ open: boolean; query: string; found: boolean }>(
    { open: false, query: "", found: true },
  );

  // WebKit's find reports only matched/not (no
  // count), and searches from the current selection — so a query edit clears
  // the selection first to search again from the top.
  const runFind = useCallback(
    (text: string, backwards: boolean, fromTop: boolean) => {
      if (!tab) return;
      if (!text) {
        browser.findClear(tab.id).catch(() => {});
        setFind((f) => ({ ...f, found: true }));
        return;
      }
      const search = () => browser.find(tab.id, text, backwards).catch(() => {});
      if (fromTop) browser.findClear(tab.id).then(search).catch(search);
      else search();
    },
    [tab],
  );

  useTauriEvent<FindResult>("browser-find", (e) => {
    if (e.payload.id !== id) return;
    setFind((f) =>
      e.payload.query === f.query ? { ...f, found: e.payload.found } : f,
    );
  });

  const closeFind = useCallback(() => {
    setFind({ open: false, query: "", found: true });
    if (tab) browser.findClear(tab.id).catch(() => {});
  }, [tab]);

  // ⌘F / ⌘G / ⇧⌘G are menu events, routed by `lib/menu/find.ts`.
  const openFind = () => {
    setFind((f) => ({ ...f, open: true }));
    // After mount; select so a second ⌘F replaces the query.
    requestAnimationFrame(() => findRef.current?.select());
  };
  const stepFind = (backwards: boolean) => {
    if (!find.open) openFind();
    else if (find.query) runFind(find.query, backwards, false);
  };
  // The pane's page-level target, so ⌘F reaches it while the native page
  // holds the keyboard (`lib/menu/find.ts`).
  useFindTarget(rootRef, { open: openFind, step: stepFind }, paneId);

  return { findRef, find, setFind, runFind, closeFind, openFind };
}

import { useRef, type Dispatch, type RefObject, type SetStateAction } from "react";
import { selectContents, useFindTarget } from "@/lib/menu/find";
import { FIND_CLOSED } from "@/components/files/pdf/constants";
import type { Engine, Find } from "@/components/files/pdf/types";

interface PdfFindArgs {
  find: Find;
  setFind: Dispatch<SetStateAction<Find>>;
  engineRef: RefObject<Engine | null>;
  /** The whole viewer, toolbar included: what ⌘F engagement is judged on. */
  rootRef: RefObject<HTMLDivElement | null>;
  containerRef: RefObject<HTMLDivElement | null>;
  viewerElRef: RefObject<HTMLDivElement | null>;
}

/**
 * pdf.js's `PDFFindController` searches and highlights; the bar only
 * dispatches its `find` events and shows the status it reports back.
 */
export function usePdfFind({
  find,
  setFind,
  engineRef,
  rootRef,
  containerRef,
  viewerElRef,
}: PdfFindArgs) {
  const findRef = useRef<HTMLInputElement>(null);

  const runFind = (query: string, again: boolean, findPrevious = false) => {
    const viewer = engineRef.current?.viewer;
    viewer?.eventBus.dispatch("find", {
      source: viewer,
      // "" debounces and restarts from the current page; "again" steps.
      type: again ? "again" : "",
      query,
      caseSensitive: false,
      entireWord: false,
      highlightAll: true,
      findPrevious,
      matchDiacritics: false,
    });
  };

  const closeFind = () => {
    const viewer = engineRef.current?.viewer;
    viewer?.eventBus.dispatch("findbarclose", { source: viewer });
    setFind(FIND_CLOSED);
  };

  /** Seeds the query from a text selection in the pages, then selects the
   *  field, so a repeat ⌘F replaces what is there. */
  const openFind = () => {
    const container = containerRef.current;
    const selection = window.getSelection();
    const picked =
      container &&
      selection &&
      !selection.isCollapsed &&
      selection.rangeCount &&
      container.contains(selection.getRangeAt(0).commonAncestorContainer)
        ? selection.toString().replace(/\s+/g, " ").trim()
        : "";
    if (picked && picked !== find.query) {
      setFind({ open: true, query: picked });
      runFind(picked, false);
    } else {
      setFind((f) => (f.open ? f : { ...f, open: true }));
    }
    // After mount when the bar was closed.
    requestAnimationFrame(() => findRef.current?.select());
  };

  const stepFind = (backwards: boolean) => {
    if (!find.open) openFind();
    else if (find.query) runFind(find.query, true, backwards);
  };

  // A PDF can be open in several tabs and side panels at once;
  // `lib/menu/find.ts` picks the one ⌘F reaches. Select All takes the pages, not
  // the toolbar.
  useFindTarget(rootRef, {
    open: openFind,
    step: stepFind,
    selectAll: () => {
      if (viewerElRef.current) selectContents(viewerElRef.current);
    },
  });

  return { findRef, runFind, closeFind };
}

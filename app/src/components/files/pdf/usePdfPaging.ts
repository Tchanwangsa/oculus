import { useCallback, useEffect, type RefObject } from "react";
import type { Engine, LayoutMode } from "@/components/files/pdf/types";

/** Page-to-page movement: the buttons, the page box's jump, and the arrow
 *  keys in the paged layouts. */
export function usePdfPaging(engineRef: RefObject<Engine | null>, mode: LayoutMode) {
  const prev = useCallback(() => engineRef.current?.viewer.previousPage(), []);
  const next = useCallback(() => engineRef.current?.viewer.nextPage(), []);

  /** The page box's Enter. Setting `currentPageNumber` scrolls the page to the
   *  top even when it is already current, and in a spread pdf.js shows the
   *  spread holding it. Anything but a whole number just reverts. */
  const jumpTo = (text: string) => {
    const viewer = engineRef.current?.viewer;
    const trimmed = text.trim();
    if (!viewer?.pagesCount || !/^\d+$/.test(trimmed)) return;
    viewer.currentPageNumber = Math.min(
      Math.max(1, Number(trimmed)),
      viewer.pagesCount,
    );
  };

  // Arrow keys page in paged layouts; in scroll layout the list scrolls
  // natively, so the keys are left alone.
  useEffect(() => {
    if (mode === "scroll") return;
    const handler = (e: KeyboardEvent) => {
      const tag = (e.target as HTMLElement)?.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;
      if (e.key === "ArrowLeft" || e.key === "ArrowUp") {
        e.preventDefault();
        prev();
      } else if (e.key === "ArrowRight" || e.key === "ArrowDown") {
        e.preventDefault();
        next();
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [mode, prev, next]);

  return { prev, next, jumpTo };
}

import { useEffect } from "react";
import type { LayoutMode } from "./layout";

/** Arrow keys page in paged layouts; in scroll layout the list scrolls
 *  natively, so the keys are left alone. */
export function usePdfPageKeys(mode: LayoutMode, prev: () => void, next: () => void) {
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
}

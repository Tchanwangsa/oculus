import { useEffect, useRef } from "react";
import { useTabActive } from "@/components/tabs/TabContext";
import { animationRuns, SHIMMER_STATE, type AnimationGate } from "@/lib/ui/visibleAnimation";

const REDUCED = "(prefers-reduced-motion: reduce)";

/** The nearest ancestor that scrolls vertically: the observer's root, so a
 *  row scrolled out of the timeline pauses. Null (the viewport) without one. */
function scroller(el: HTMLElement): HTMLElement | null {
  for (let p = el.parentElement; p; p = p.parentElement) {
    const y = getComputedStyle(p).overflowY;
    if (y === "auto" || y === "scroll") return p;
  }
  return null;
}

/**
 * Pauses the shimmer on the returned ref's element while nobody can see it,
 * by writing `--shimmer-state` (`running` or `paused`) on it. Only the play
 * state changes: no `will-change` or layer toggle, which would jitter icons at
 * fractional zoom (docs/ui.md). The label itself is observed, since
 * `useRowVisibility` can move a row's host out of the document; the observer
 * goes with the element on unmount. `on` false leaves the element alone.
 */
export function useVisibleAnimation<T extends HTMLElement>(on: boolean) {
  const ref = useRef<T>(null);
  const tabActive = useTabActive();
  useEffect(() => {
    const el = ref.current;
    if (!on || !el) return;
    const motion = typeof window.matchMedia === "function" ? window.matchMedia(REDUCED) : null;
    const gate: AnimationGate = {
      intersecting: true,
      documentVisible: document.visibilityState !== "hidden",
      reducedMotion: motion?.matches ?? false,
      tabActive,
    };
    const write = () => el.style.setProperty(SHIMMER_STATE, animationRuns(gate) ? "running" : "paused");
    const onVisibility = () => {
      gate.documentVisible = document.visibilityState !== "hidden";
      write();
    };
    const onMotion = (e: MediaQueryListEvent) => {
      gate.reducedMotion = e.matches;
      write();
    };
    const observer =
      typeof IntersectionObserver === "function"
        ? new IntersectionObserver(
            (entries) => {
              const last = entries[entries.length - 1];
              if (!last) return;
              gate.intersecting = last.isIntersecting;
              write();
            },
            { root: scroller(el) },
          )
        : null;
    observer?.observe(el);
    document.addEventListener("visibilitychange", onVisibility);
    motion?.addEventListener("change", onMotion);
    write();
    return () => {
      observer?.disconnect();
      document.removeEventListener("visibilitychange", onVisibility);
      motion?.removeEventListener("change", onMotion);
      el.style.removeProperty(SHIMMER_STATE);
    };
  }, [on, tabActive]);
  return ref;
}

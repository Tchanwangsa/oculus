import { useEffect, type RefObject } from "react";

interface KeyboardShortcutsArgs {
  focused: boolean;
  toggleFullscreen: () => void;
  isFullscreenRef: RefObject<boolean>;
  videoRef: RefObject<HTMLVideoElement | null>;
  // The handler registers once and reaches actions through refs.
  elsewhereRef: RefObject<boolean>;
  claimRef: RefObject<((at?: number) => void) | undefined>;
  togglePlayRef: RefObject<() => void>;
  toggleDockRef: RefObject<() => void>;
  toggleCaptionsRef: RefObject<() => void>;
}

/** The focused pane's player only. While `elsewhere`, Space plays here and
 *  Escape still leaves fullscreen; every other key waits for ownership. */
export function useKeyboardShortcuts({
  focused,
  toggleFullscreen,
  isFullscreenRef,
  videoRef,
  elsewhereRef,
  claimRef,
  togglePlayRef,
  toggleDockRef,
  toggleCaptionsRef,
}: KeyboardShortcutsArgs) {
  useEffect(() => {
    if (!focused) return;
    const handler = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement | null;
      const tag = target?.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;
      // An open popover (speed, layout, source) owns its arrows and space bar.
      if (target?.closest('[data-slot="popover-content"]')) return;

      if (elsewhereRef.current && e.key !== "Escape") {
        if (e.key === " ") {
          e.preventDefault();
          claimRef.current?.();
        }
        return;
      }

      switch (e.key) {
        case " ":
          e.preventDefault();
          togglePlayRef.current();
          break;
        case "ArrowLeft":
          e.preventDefault();
          if (videoRef.current)
            videoRef.current.currentTime = Math.max(
              0,
              videoRef.current.currentTime - 5,
            );
          break;
        case "ArrowRight":
          e.preventDefault();
          if (videoRef.current) videoRef.current.currentTime += 5;
          break;
        case "f":
          if (!e.ctrlKey && !e.metaKey && !e.altKey) {
            e.preventDefault();
            toggleFullscreen();
          }
          break;
        // Bare keys only: ⌘T opens a tab and ⌃C is a copy on some layouts.
        case "c":
          if (!e.ctrlKey && !e.metaKey && !e.altKey) {
            e.preventDefault();
            toggleCaptionsRef.current();
          }
          break;
        case "t":
          if (!e.ctrlKey && !e.metaKey && !e.altKey) {
            e.preventDefault();
            toggleDockRef.current();
          }
          break;
        case "Escape":
          if (isFullscreenRef.current) {
            e.preventDefault();
            toggleFullscreen();
          }
          break;
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [toggleFullscreen, focused]);
}

import { useEffect } from "react";

import type { EditorMode } from "../DocumentControls";

/** ⌘S saves and ⌘⇧P flips Live/Raw, on the document so they work from the
 *  title field too. */
export function useNoteShortcuts(
  tabActive: boolean,
  flush: () => Promise<void>,
  mode: EditorMode,
  onMode: (mode: EditorMode) => void,
) {
  useEffect(() => {
    if (!tabActive) return;
    const onKey = (e: KeyboardEvent) => {
      if (!(e.metaKey || e.ctrlKey) || e.altKey) return;
      const key = e.key.toLowerCase();
      if (key === "s" && !e.shiftKey) {
        e.preventDefault();
        void flush();
      } else if (key === "p" && e.shiftKey) {
        e.preventDefault();
        onMode(mode === "live" ? "raw" : "live");
      }
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [tabActive, flush, mode, onMode]);
}

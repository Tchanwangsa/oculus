import { useEffect, useRef, useState } from "react";

import type { WorkspaceSaver } from "@/lib/ed/workspaceSaver";

/**
 * An action that replaces every file of the workspace with Ed's (reset,
 * restore), behind a confirmation. Confirming drops the saver's buffers
 * first, unsaved text included, and waits out a write in flight, so no save
 * of the old text lands after Ed's files; the `reset` event then reloads the
 * pane. A failure shows; nothing retries by itself.
 */
export function useReplaceFlow<T>(saver: WorkspaceSaver | null, replace: (target: T) => Promise<void>) {
  const [asking, setAsking] = useState<T | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const live = useRef(true);
  useEffect(() => {
    live.current = true;
    return () => {
      live.current = false;
    };
  }, []);

  async function confirm() {
    if (busy || asking == null || !saver) return;
    const target = asking;
    setBusy(true);
    setError(null);
    try {
      await saver.discard();
      await replace(target);
      if (live.current) setAsking(null);
    } catch (e) {
      if (!live.current) return;
      setAsking(null);
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      if (live.current) setBusy(false);
    }
  }

  return {
    /** What the dialog asks about; null when closed. */
    asking,
    busy,
    error,
    ask: (target: T) => {
      setError(null);
      setAsking(target);
    },
    cancel: () => !busy && setAsking(null),
    confirm: () => void confirm(),
    dismissError: () => setError(null),
  };
}

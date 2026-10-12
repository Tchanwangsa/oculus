import { useCallback, useEffect, useState } from "react";

import { listThemes, type ThemeInfo } from "@/lib/studio/themes";

export interface ThemesState {
  themes: ThemeInfo[];
  loading: boolean;
  /** Why the list could not be read; the chip then offers only Default. */
  error: string | null;
}

/** The themes for `subject` (a course code), re-read when it changes or on
 *  `reload` (after a write or an agent's turn). */
export function useThemes(subject: string | null): ThemesState & { reload: () => void } {
  const [state, setState] = useState<ThemesState>({ themes: [], loading: true, error: null });
  const [tick, setTick] = useState(0);
  useEffect(() => {
    let stale = false;
    setState((s) => ({ ...s, loading: true }));
    listThemes(subject)
      .then((themes) => !stale && setState({ themes, loading: false, error: null }))
      .catch((e) => !stale && setState({ themes: [], loading: false, error: e instanceof Error ? e.message : String(e) }));
    return () => {
      stale = true;
    };
  }, [subject, tick]);
  const reload = useCallback(() => setTick((n) => n + 1), []);
  return { ...state, reload };
}

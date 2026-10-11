import { useEffect, useState } from "react";

import { listThemes, type ThemeInfo } from "@/lib/studio/themes";

export interface ThemesState {
  themes: ThemeInfo[];
  loading: boolean;
  /** Why the list could not be read; the chip then offers only Default. */
  error: string | null;
}

/** The themes for `subject` (a course code), re-read when it changes. */
export function useThemes(subject: string | null): ThemesState {
  const [state, setState] = useState<ThemesState>({ themes: [], loading: true, error: null });
  useEffect(() => {
    let stale = false;
    setState((s) => ({ ...s, loading: true }));
    listThemes(subject)
      .then((themes) => !stale && setState({ themes, loading: false, error: null }))
      .catch((e) => !stale && setState({ themes: [], loading: false, error: e instanceof Error ? e.message : String(e) }));
    return () => {
      stale = true;
    };
  }, [subject]);
  return state;
}

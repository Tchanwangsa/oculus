import { useCallback, useEffect, useRef, useState } from "react";

import { listComponents, type ComponentInfo } from "@/lib/studio/components";

export interface ComponentsState {
  components: ComponentInfo[];
  loaded: boolean;
  /** Why the list could not be read. */
  error: string | null;
  /** Reads the list again, keeping the rows on screen until it lands. */
  reload: () => Promise<void>;
}

/** The components over the cascade for `subject` (a course code; null is the
 *  global library), read again when it changes. Latest read wins. */
export function useComponents(subject: string | null): ComponentsState {
  const [state, setState] = useState<Omit<ComponentsState, "reload">>({
    components: [],
    loaded: false,
    error: null,
  });
  const run = useRef(0);

  const reload = useCallback(async () => {
    const mine = ++run.current;
    try {
      const components = await listComponents(subject);
      if (mine === run.current) setState({ components, loaded: true, error: null });
    } catch (e) {
      if (mine === run.current) {
        setState({ components: [], loaded: true, error: e instanceof Error ? e.message : String(e) });
      }
    }
  }, [subject]);

  useEffect(() => {
    setState({ components: [], loaded: false, error: null });
    void reload();
  }, [reload]);

  return { ...state, reload };
}

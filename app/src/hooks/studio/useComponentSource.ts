import { useEffect, useState } from "react";

import { componentSource, type ComponentText } from "@/lib/studio/components";

export interface SourceState {
  source: ComponentText | null;
  /** Why the source could not be read. */
  error: string | null;
}

/** The component's TSX from its nearest level. `revision` re-reads it after
 *  a change; the old text stays up until the new one lands. */
export function useComponentSource(name: string, subject: string | null, revision: number): SourceState {
  const [state, setState] = useState<SourceState>({ source: null, error: null });
  useEffect(() => {
    let stale = false;
    componentSource(name, subject)
      .then((source) => !stale && setState({ source, error: null }))
      .catch((e) => !stale && setState({ source: null, error: e instanceof Error ? e.message : String(e) }));
    return () => {
      stale = true;
    };
  }, [name, subject, revision]);
  return state;
}

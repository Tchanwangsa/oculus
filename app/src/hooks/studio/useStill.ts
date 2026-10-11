import { useEffect, useState } from "react";

import { stillUrl } from "@/lib/studio/stills";

export interface Still {
  url: string | null;
  /** Why it could not be made: a sentence fit to show. */
  error: string | null;
}

/** A still from `load`, cached under `key` (null asks for nothing). */
export function useStill(key: string | null, load: () => Promise<Blob>): Still {
  const [still, setStill] = useState<Still>({ url: null, error: null });
  useEffect(() => {
    if (key == null) return;
    let stale = false;
    setStill({ url: null, error: null });
    stillUrl(key, load)
      .then((url) => !stale && setStill({ url, error: null }))
      .catch((e) => !stale && setStill({ url: null, error: e instanceof Error ? e.message : String(e) }));
    return () => {
      stale = true;
    };
    // `load` is a closure over what `key` names.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key]);
  return still;
}

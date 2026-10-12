import { useEffect, useState } from "react";

import { readSetupProblems, type SetupProblem } from "@/lib/studio/setup";
import type { VoiceSettings } from "@/lib/studio/voice";

/** What is still missing before a video can be narrated, read on mount;
 *  `undefined` while it loads, `null` when it could not be read. */
export function useSetupProblems(): { voice: VoiceSettings; problems: SetupProblem[] } | null | undefined {
  const [state, setState] = useState<{ voice: VoiceSettings; problems: SetupProblem[] } | null | undefined>(undefined);
  useEffect(() => {
    let stale = false;
    readSetupProblems()
      .then((s) => !stale && setState(s))
      .catch(() => !stale && setState(null));
    return () => {
      stale = true;
    };
  }, []);
  return state;
}

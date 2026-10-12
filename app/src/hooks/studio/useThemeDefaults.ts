import { useCallback, useEffect, useState } from "react";

import type { ComponentLevel } from "@/lib/studio/components";
import { setDefaultTheme, themeDefaults, type ThemeDefaults } from "@/lib/studio/themes";

const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

/** The default themes of the global library and of `subject`'s, and the one
 *  a new video there takes; `set` writes a library's `library.json`. */
export function useThemeDefaults(subject: string | null) {
  const [defaults, setDefaults] = useState<ThemeDefaults | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [tick, setTick] = useState(0);

  useEffect(() => {
    let stale = false;
    themeDefaults(subject)
      .then((d) => {
        if (stale) return;
        setDefaults(d);
        setError(null);
      })
      .catch((e) => !stale && setError(message(e)));
    return () => {
      stale = true;
    };
  }, [subject, tick]);

  const set = useCallback(
    async (level: ComponentLevel, name: string | null) => {
      setError(null);
      try {
        setDefaults(await setDefaultTheme(level, subject, name));
      } catch (e) {
        setError(message(e));
      }
    },
    [subject],
  );
  const reload = useCallback(() => setTick((n) => n + 1), []);
  return { defaults, error, set, reload };
}

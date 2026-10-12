import { useEffect, useState } from "react";

import type { ComponentLevel, Snapshot } from "@/lib/studio/components";
import { themeHistory, themeSource, type ThemeText } from "@/lib/studio/themes";

const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

export interface ThemeSourceState {
  source: ThemeText | null;
  /** Why the text could not be read. */
  error: string | null;
}

/** The theme's file text from the level that wins. `revision` re-reads it
 *  after a write or an agent's turn; the old text stays until the new one
 *  lands. */
export function useThemeSource(name: string, subject: string | null, revision: number): ThemeSourceState {
  const [state, setState] = useState<ThemeSourceState>({ source: null, error: null });
  useEffect(() => {
    let stale = false;
    themeSource(name, subject)
      .then((source) => !stale && setState({ source, error: null }))
      .catch((e) => !stale && setState({ source: null, error: message(e) }));
    return () => {
      stale = true;
    };
  }, [name, subject, revision]);
  return state;
}

/** The snapshots of a library theme at `level`, newest first; empty for a
 *  bundled theme (`level` null), which has none. */
export function useThemeHistory(
  name: string,
  level: ComponentLevel | null,
  subject: string | null,
  revision: number,
): Snapshot[] {
  const [snapshots, setSnapshots] = useState<Snapshot[]>([]);
  useEffect(() => {
    let stale = false;
    if (!level) {
      setSnapshots([]);
      return;
    }
    themeHistory(name, level, level === "subject" ? subject : null)
      .then((s) => !stale && setSnapshots(s))
      .catch(() => !stale && setSnapshots([]));
    return () => {
      stale = true;
    };
  }, [name, level, subject, revision]);
  return snapshots;
}

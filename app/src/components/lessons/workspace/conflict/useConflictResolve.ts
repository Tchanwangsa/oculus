import { useEffect, useRef, useState } from "react";

import { flushForEd } from "@/lib/ed/flushForEd";
import { readWorkspaceFile, resolveConflict } from "@/lib/ed/workspace";
import type { WorkspaceSaver } from "@/lib/ed/workspaceSaver";

export type Keep = "mine" | "ed";

/** Both versions of the file, for Compare: yours as the editor holds it. */
export interface ConflictTexts {
  mine: string;
  eds: string;
}

export interface ConflictError {
  title: string;
  message: string;
}

/**
 * Compare, Keep mine and Take Ed's for one file in conflict. Each resolve
 * writes pending saves first; Take Ed's then reads the replaced file into the
 * buffer over whatever was typed (`saver.replace`). Nothing retries by itself.
 */
export function useConflictResolve({
  challengeId,
  saver,
  path,
  serverPath,
  onResolved,
}: {
  challengeId: number;
  saver: WorkspaceSaver;
  path: string;
  /** Ed's copy beside the file; null when the file is in no conflict. */
  serverPath: string | null;
  onResolved: (path: string) => void;
}) {
  const [texts, setTexts] = useState<ConflictTexts | "loading" | null>(null);
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState<Keep | null>(null);
  const [error, setError] = useState<ConflictError | null>(null);
  const live = useRef(true);

  useEffect(() => {
    live.current = true;
    return () => {
      live.current = false;
    };
  }, []);

  // A new conflict (or none) starts from the editor again.
  useEffect(() => {
    setTexts(null);
    setConfirming(false);
    setError(null);
  }, [serverPath]);

  async function compare() {
    if (texts != null) return setTexts(null);
    if (serverPath == null) return;
    setTexts("loading");
    setError(null);
    try {
      const [mine, eds] = await Promise.all([
        saver.text(path) ?? readWorkspaceFile(challengeId, path),
        readWorkspaceFile(challengeId, serverPath),
      ]);
      if (live.current) setTexts({ mine, eds });
    } catch (e) {
      if (!live.current) return;
      setTexts(null);
      setError({ title: "Couldn't compare the versions", message: String(e) });
    }
  }

  async function resolve(keep: Keep) {
    if (busy || serverPath == null) return;
    setConfirming(false);
    setBusy(keep);
    setError(null);
    try {
      if (keep === "mine") {
        const why = await flushForEd(saver);
        if (why) throw new Error(why);
      } else {
        // Only so no write of the discarded text is still in flight.
        await saver.flush();
      }
      await resolveConflict(challengeId, path, keep);
      if (keep === "ed") saver.replace(path, await readWorkspaceFile(challengeId, path));
      if (!live.current) return;
      setTexts(null);
      onResolved(path);
    } catch (e) {
      if (!live.current) return;
      const title = keep === "mine" ? "Couldn't keep your version" : "Couldn't take Ed's version";
      setError({ title, message: e instanceof Error ? e.message : String(e) });
    } finally {
      if (live.current) setBusy(null);
    }
  }

  return {
    texts,
    busy,
    error,
    confirming,
    compare: () => void compare(),
    keepMine: () => void resolve("mine"),
    askTakeEds: () => setConfirming(true),
    takeEds: () => void resolve("ed"),
    cancelTakeEds: () => setConfirming(false),
  };
}

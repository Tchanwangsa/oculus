import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

/**
 * Whether a key is saved, asked once on mount through a `*_has_api_key`
 * command. `hasKey` is `null` until answered and stays `null` when the check
 * fails (`checkError` says why): a failed check is not a missing key.
 */
export function useKeyPresence(command: string) {
  const [hasKey, setHasKey] = useState<boolean | null>(null);
  const [checkError, setCheckError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    invoke<boolean>(command)
      .then((present) => {
        if (cancelled) return;
        setHasKey(present);
        setCheckError(null);
      })
      .catch((cause) => {
        console.error(`${command} failed`, cause);
        if (cancelled) return;
        setHasKey(null);
        setCheckError(String(cause));
      });
    return () => {
      cancelled = true;
    };
  }, [command]);

  return { hasKey, setHasKey, checkError };
}

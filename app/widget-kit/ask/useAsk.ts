import { useCallback, useEffect, useRef, useState } from "react";
import type { JsonSchema } from "./schema";
import { ask as askOnce, send as sendOnce } from "./calls";

/**
 * `ask` and `send` for a component: `busy` while one runs, `error` the last
 * refusal or failure (cleared by the next call). These never reject: a
 * failure resolves `undefined` and sets `error`, so a widget can show it
 * without a `catch`.
 */
export function useAsk() {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);

  const track = useCallback(async <T,>(call: () => Promise<T>): Promise<T | undefined> => {
    setBusy(true);
    setError(null);
    try {
      return await call();
    } catch (e) {
      if (mounted.current) setError(e instanceof Error ? e.message : String(e));
      return undefined;
    } finally {
      if (mounted.current) setBusy(false);
    }
  }, []);

  const ask = useCallback(<T,>(prompt: string, schema: JsonSchema) => track(() => askOnce<T>(prompt, schema)), [track]);
  const send = useCallback((text: string) => track(() => sendOnce(text)), [track]);
  return { ask, send, busy, error };
}

import { useCallback, useRef, useState } from "react";
import { commandError } from "@/lib/harness";

/**
 * One command a control sends (Stop, Resume, Force stop): `pending` while it
 * is out, so the control is inert and a second click sends nothing, and the
 * failure's reason in `error` until the next try. It always comes back from
 * pending, whatever the command did.
 */
export function useRunCommand() {
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const out = useRef(false);
  const run = useCallback(async (command: () => Promise<unknown>): Promise<boolean> => {
    if (out.current) return false;
    out.current = true;
    setPending(true);
    setError(null);
    try {
      await command();
      return true;
    } catch (e) {
      setError(commandError(e));
      return false;
    } finally {
      out.current = false;
      setPending(false);
    }
  }, []);
  const clear = useCallback(() => setError(null), []);
  return { pending, error, run, clear };
}

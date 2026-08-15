import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

export interface KeepaliveStatus {
  /** False off macOS — LaunchAgents are the only backend implemented. */
  supported: boolean;
  enabled: boolean;
  interval_hours: number;
  /** Last line the agent logged, so the UI can prove it is running. */
  last_run: string | null;
}

/** Background Canvas session refresh, run by launchd so it survives the app
 *  being closed. */
export function useKeepalive() {
  const [status, setStatus] = useState<KeepaliveStatus | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setStatus(await invoke<KeepaliveStatus>("keepalive_status"));
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const toggle = useCallback(
    async (enabled: boolean, intervalHours = 6) => {
      setBusy(true);
      setError(null);
      try {
        if (enabled) {
          await invoke("keepalive_enable", { intervalHours });
        } else {
          await invoke("keepalive_disable");
        }
        await refresh();
      } catch (e) {
        setError(String(e));
      } finally {
        setBusy(false);
      }
    },
    [refresh],
  );

  return { status, busy, error, toggle, refresh };
}

import { useEffect, useState, useSyncExternalStore } from "react";
import { browserShareEnv } from "@/lib/companion/env";
import type { ShareCredentials } from "@/lib/companion/link";
import { ShareClient, type ShareClientState } from "@/lib/companion/shareClient";

/** One client per mounted page; StrictMode's second mount gets a fresh one. */
export function useShareClient(credentials: ShareCredentials): { client: ShareClient | null; state: ShareClientState | null } {
  const [client, setClient] = useState<ShareClient | null>(null);
  useEffect(() => {
    const next = new ShareClient(credentials, browserShareEnv());
    setClient(next);
    next.start();
    return () => next.dispose();
  }, [credentials]);
  const state = useSyncExternalStore(
    (listener) => client?.subscribe(listener) ?? (() => {}),
    () => client?.getState() ?? null,
  );
  return { client, state };
}

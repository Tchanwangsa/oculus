import { useEffect } from "react";
import { useIndexStore } from "@/stores/sync/indexStore";
import { embedReady } from "@/lib/pipeline/retrieval";
import { useEmbedEvents } from "./useEmbedEvents";
import { useHarnessEvents } from "./useHarnessEvents";
import { useParseEvents } from "./useParseEvents";
import { useScrapeEvents } from "./useScrapeEvents";

/**
 * The one bridge from backend Tauri events into the global stores and the DB.
 * Mount once at the app root so progress survives navigation; UI reads the
 * stores and never listens directly.
 */
export function useBackendEvents() {
  // Re-asked by Settings → Embeddings when a key is saved or cleared.
  useEffect(() => {
    void embedReady().then((ready) => useIndexStore.getState().setReady(ready));
  }, []);

  useHarnessEvents();
  useScrapeEvents();
  useParseEvents();
  useEmbedEvents();
}

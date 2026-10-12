import { embedReady } from "@/lib/pipeline/retrieval";
import { useIndexStore } from "@/stores/sync/indexStore";
import { useCloudKey } from "./useCloudKey";

/**
 * The Voyage API key's field (Settings → Search index, onboarding's Library
 * step). Whether a key is saved is the caller's: Settings reads
 * `embed_settings`' `credentials_ready`, onboarding asks `voyage_has_api_key`
 * (`useKeyPresence`). `onSaved` / `onRemoved` run before the index store
 * hears of it and before the note.
 */
export function useVoyageKey({
  onSaved,
  onRemoved,
}: { onSaved?: () => void; onRemoved?: () => void } = {}) {
  return useCloudKey({
    service: "Voyage",
    // Rust checks the key against Voyage before storing it.
    setCommand: "voyage_set_api_key",
    deleteCommand: "voyage_delete_api_key",
    onSaved: () => {
      onSaved?.();
      // Re-asked rather than assumed: the engine also decides readiness.
      void embedReady().then((ready) => useIndexStore.getState().setReady(ready));
    },
    onRemoved: () => {
      onRemoved?.();
      useIndexStore.getState().setReady(false);
    },
  });
}

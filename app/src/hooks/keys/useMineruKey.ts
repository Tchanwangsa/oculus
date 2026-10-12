import { sweepNow } from "@/hooks/sync/useQualitySweep";
import { useParseStore } from "@/stores/sync/parseStore";
import { useCloudKey } from "./useCloudKey";
import { useKeyPresence } from "./useKeyPresence";

/**
 * The MinerU API token's field (Settings → Parsing, onboarding's Library
 * step). `hasKey` comes from `mineru_has_api_key`. `onSaved` / `onRemoved`
 * let a caller update its own settings copy; they run after `hasKey` moves
 * and before the parse side effects and the note.
 */
export function useMineruKey({
  onSaved,
  onRemoved,
}: { onSaved?: () => void; onRemoved?: () => void } = {}) {
  const { hasKey, setHasKey, checkError } = useKeyPresence("mineru_has_api_key");
  const field = useCloudKey({
    service: "MinerU",
    // Rust checks the token against MinerU before storing it.
    setCommand: "mineru_set_api_key",
    deleteCommand: "mineru_delete_api_key",
    onSaved: () => {
      setHasKey(true);
      onSaved?.();
      // An accepted token lifts the latch and frees the files that failed
      // only for want of one; the sweep then resumes outstanding files now.
      useParseStore.getState().tokenSaved();
      sweepNow();
    },
    onRemoved: () => {
      setHasKey(false);
      onRemoved?.();
    },
  });
  return { ...field, hasKey, checkError };
}

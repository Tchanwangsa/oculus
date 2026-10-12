import { useState, type Dispatch, type SetStateAction } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { ReindexPrompt } from "../ReindexConfirmDialog";
import type { EmbedSettings } from "./types";

interface EmbedActionsInput {
  settings: EmbedSettings | null;
  setSettings: Dispatch<SetStateAction<EmbedSettings | null>>;
  setError: Dispatch<SetStateAction<string | null>>;
  loadEstimate: () => void;
}

/** The engine switch and the spend guard: every write on this page but the
 *  Voyage key, which is `useVoyageKey`'s. */
export function useEmbedActions({ settings, setSettings, setError, loadEstimate }: EmbedActionsInput) {
  const [prompt, setPrompt] = useState<(ReindexPrompt & { engine: string }) | null>(null);
  const [switching, setSwitching] = useState(false);

  const apply = async (engine: string) => {
    setSwitching(true);
    setError(null);
    try {
      setSettings(await invoke<EmbedSettings>("embed_set_engine", { engine }));
      setPrompt(null);
      loadEstimate();
    } catch (cause) {
      console.error("embed engine change failed", cause);
      setError(String(cause));
    } finally {
      setSwitching(false);
    }
  };

  // An empty index has nothing to lose, so it skips the confirmation.
  const choose = (engine: string) => {
    if (!settings || engine === settings.engine) return;
    const { pages_embedded, files_embedded, model } = settings.index;
    if (pages_embedded === 0) {
      void apply(engine);
      return;
    }
    setPrompt({
      engine,
      to: settings.engines.find((option) => option.id === engine)?.label ?? engine,
      from: model,
      vectors: pages_embedded,
      files: files_embedded,
    });
  };

  // The guard moves where the run stops, so re-measure the estimate.
  const setBudget = async (percent: number) => {
    try {
      setSettings(await invoke<EmbedSettings>("embed_set_budget", { percent }));
      loadEstimate();
    } catch (cause) {
      console.error("embed budget change failed", cause);
      setError(String(cause));
    }
  };

  return {
    prompt,
    setPrompt,
    switching,
    apply,
    choose,
    setBudget,
  };
}

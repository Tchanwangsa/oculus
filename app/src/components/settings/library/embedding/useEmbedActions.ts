import { useState, type Dispatch, type SetStateAction } from "react";
import { invoke } from "@tauri-apps/api/core";
import { embedReady } from "@/lib/pipeline/retrieval";
import { useIndexStore } from "@/stores/sync/indexStore";
import type { ReindexPrompt } from "../ReindexConfirmDialog";
import type { EmbedSettings } from "./types";

interface EmbedActionsInput {
  settings: EmbedSettings | null;
  setSettings: Dispatch<SetStateAction<EmbedSettings | null>>;
  setError: Dispatch<SetStateAction<string | null>>;
  loadEstimate: () => void;
}

/** The engine switch, the spend guard and the Voyage key: every write on this page. */
export function useEmbedActions({ settings, setSettings, setError, loadEstimate }: EmbedActionsInput) {
  const [prompt, setPrompt] = useState<(ReindexPrompt & { engine: string }) | null>(null);
  const [switching, setSwitching] = useState(false);

  const [key, setKey] = useState("");
  const [keyNote, setKeyNote] = useState<{ kind: "error" | "warn"; text: string } | null>(null);
  const [checkingKey, setCheckingKey] = useState(false);

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

  // Rust checks the key against Voyage before storing it in the keychain.
  const saveKey = async () => {
    if (!key.trim()) return;
    setCheckingKey(true);
    setKeyNote(null);
    try {
      const verdict = await invoke<string>("voyage_set_api_key", { key: key.trim() });
      setKey("");
      setSettings((prev) => (prev ? { ...prev, credentials_ready: true, credentials_error: null } : prev));
      // Re-asked rather than assumed: the engine also decides readiness.
      void embedReady().then((ready) => useIndexStore.getState().setReady(ready));
      setKeyNote(
        verdict === "unverified"
          ? { kind: "warn", text: "Saved, but Voyage was unreachable — it has not been checked." }
          : null,
      );
    } catch (cause) {
      setKeyNote({ kind: "error", text: String(cause) });
    } finally {
      setCheckingKey(false);
    }
  };

  const deleteKey = async () => {
    setKeyNote(null);
    try {
      await invoke("voyage_delete_api_key");
      setKey("");
      setSettings((prev) => (prev ? { ...prev, credentials_ready: false, credentials_error: null } : prev));
      useIndexStore.getState().setReady(false);
    } catch (cause) {
      console.error("Voyage key removal failed", cause);
      setKeyNote({ kind: "error", text: String(cause) });
    }
  };

  return {
    prompt,
    setPrompt,
    switching,
    key,
    setKey,
    keyNote,
    checkingKey,
    apply,
    choose,
    setBudget,
    saveKey,
    deleteKey,
  };
}

import { useState, type Dispatch, type SetStateAction } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { ParseSettings } from "./types";

interface ParseActionsInput {
  settings: ParseSettings | null;
  applySettings: (next: ParseSettings) => void;
  setError: Dispatch<SetStateAction<string | null>>;
  urlDraft: string;
  runProbe: (url?: string) => Promise<void>;
}

/** The engine switch, endpoint and certificate toggle: every write on this
 *  page but the MinerU token, which is `useMineruKey`'s. */
export function useParseActions({
  settings,
  applySettings,
  setError,
  urlDraft,
  runProbe,
}: ParseActionsInput) {
  const [switching, setSwitching] = useState(false);

  const [urlNote, setUrlNote] = useState<string | null>(null);
  const [savingUrl, setSavingUrl] = useState(false);

  const [savingCert, setSavingCert] = useState(false);
  const [certError, setCertError] = useState<string | null>(null);

  const choose = async (engine: string) => {
    if (!settings || engine === settings.engine) return;
    setSwitching(true);
    setError(null);
    try {
      applySettings(await invoke<ParseSettings>("parse_set_engine", { engine }));
    } catch (cause) {
      console.error("parse engine change failed", cause);
      setError(String(cause));
    } finally {
      setSwitching(false);
    }
  };

  // Not guarded on non-empty: an empty field clears the override.
  const saveUrl = async () => {
    setUrlNote(null);
    setSavingUrl(true);
    try {
      applySettings(await invoke<ParseSettings>("parse_set_engine_url", { url: urlDraft.trim() }));
      await runProbe();
    } catch (cause) {
      console.error("parse endpoint change failed", cause);
      setUrlNote(String(cause));
    } finally {
      setSavingUrl(false);
    }
  };

  const setAcceptExpired = async (accept: boolean) => {
    setSavingCert(true);
    setCertError(null);
    try {
      applySettings(
        await invoke<ParseSettings>("parse_set_accept_expired_result_cert", { accept }),
      );
    } catch (cause) {
      console.error("result certificate setting failed", cause);
      setCertError(String(cause));
    } finally {
      setSavingCert(false);
    }
  };

  return {
    switching,
    urlNote,
    savingUrl,
    savingCert,
    certError,
    choose,
    saveUrl,
    setAcceptExpired,
  };
}

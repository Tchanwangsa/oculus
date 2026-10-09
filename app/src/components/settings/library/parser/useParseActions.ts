import { useState, type Dispatch, type SetStateAction } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useParseStore } from "@/stores/sync/parseStore";
import type { ParseSettings } from "./types";

interface ParseActionsInput {
  settings: ParseSettings | null;
  applySettings: (next: ParseSettings) => void;
  setError: Dispatch<SetStateAction<string | null>>;
  setHasToken: Dispatch<SetStateAction<boolean | null>>;
  urlDraft: string;
  runProbe: (url?: string) => Promise<void>;
}

/** The engine switch, endpoint, certificate toggle and MinerU token: every write on this page. */
export function useParseActions({
  settings,
  applySettings,
  setError,
  setHasToken,
  urlDraft,
  runProbe,
}: ParseActionsInput) {
  const [switching, setSwitching] = useState(false);

  const [token, setToken] = useState("");
  const [tokenNote, setTokenNote] = useState<{ kind: "error" | "warn"; text: string } | null>(null);
  const [checkingToken, setCheckingToken] = useState(false);

  const [urlNote, setUrlNote] = useState<string | null>(null);
  const [savingUrl, setSavingUrl] = useState(false);

  const [savingCert, setSavingCert] = useState(false);
  const [certError, setCertError] = useState<string | null>(null);

  const clearLatch = useParseStore((state) => state.clearLatch);

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

  // Rust checks the token against MinerU before storing it in the keychain.
  const saveToken = async () => {
    if (!token.trim()) return;
    setCheckingToken(true);
    setTokenNote(null);
    try {
      const verdict = await invoke<string>("mineru_set_api_key", { key: token.trim() });
      setToken("");
      setHasToken(true);
      // An accepted token lifts the latch; the sweep resumes outstanding files.
      clearLatch();
      setTokenNote(
        verdict === "unverified"
          ? { kind: "warn", text: "Saved, but MinerU was unreachable — it has not been checked." }
          : null,
      );
    } catch (cause) {
      setTokenNote({ kind: "error", text: String(cause) });
    } finally {
      setCheckingToken(false);
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

  const deleteToken = async () => {
    setTokenNote(null);
    try {
      await invoke("mineru_delete_api_key");
      setHasToken(false);
      setToken("");
    } catch (cause) {
      console.error("MinerU token removal failed", cause);
      setTokenNote({ kind: "error", text: String(cause) });
    }
  };

  return {
    switching,
    token,
    setToken,
    tokenNote,
    checkingToken,
    urlNote,
    savingUrl,
    savingCert,
    certError,
    choose,
    saveUrl,
    saveToken,
    setAcceptExpired,
    deleteToken,
  };
}

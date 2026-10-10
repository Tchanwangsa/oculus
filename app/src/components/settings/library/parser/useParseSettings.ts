import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { resultCertState, type ResultCertState } from "@/lib/pipeline/resultCert";
import type { ParseSettings } from "./types";

/** The parser settings, the saved-token check and the result-CDN certificate probe. */
export function useParseSettings() {
  const [settings, setSettings] = useState<ParseSettings | null>(null);
  const [error, setError] = useState<string | null>(null);

  // `null` is "not answered yet" and must stay distinct from `false`: a check
  // that failed is not a missing token.
  const [hasToken, setHasToken] = useState<boolean | null>(null);
  const [tokenCheckError, setTokenCheckError] = useState<string | null>(null);

  // The endpoint as typed; empty means "no override" (`default_base_url`).
  const [urlDraft, setUrlDraft] = useState("");

  const [cert, setCert] = useState<ResultCertState | null>(null);

  // Rust owns the endpoint in force, so re-seed the field from every answer.
  const applySettings = useCallback((next: ParseSettings) => {
    setSettings(next);
    setUrlDraft(next.overridden ? next.base_url : "");
  }, []);

  // Loaded independently so one failing is never read as the other's answer.
  useEffect(() => {
    let cancelled = false;

    invoke<ParseSettings>("parse_settings")
      .then((next) => {
        if (cancelled) return;
        applySettings(next);
        setError(null);
      })
      .catch((cause) => {
        console.error("parse settings failed", cause);
        if (cancelled) return;
        setError("Could not read the parser settings.");
      });

    // Asked whichever engine is selected, so switching to cloud shows no flash.
    invoke<boolean>("mineru_has_api_key")
      .then((present) => {
        if (cancelled) return;
        setHasToken(present);
        setTokenCheckError(null);
      })
      .catch((cause) => {
        console.error("MinerU token check failed", cause);
        if (cancelled) return;
        setHasToken(null);
        setTokenCheckError(String(cause));
      });

    return () => {
      cancelled = true;
    };
  }, [applySettings]);

  // A bare handshake with the result CDN, so the warning is current on open.
  useEffect(() => {
    if (settings?.engine !== "cloud") return;
    let cancelled = false;
    resultCertState(true)
      .then((next) => {
        if (!cancelled) setCert(next);
      })
      .catch((cause) => console.error("result certificate probe failed", cause));
    return () => {
      cancelled = true;
    };
  }, [settings?.engine, settings?.accept_expired_result_cert]);

  return {
    settings,
    setSettings,
    applySettings,
    error,
    setError,
    hasToken,
    setHasToken,
    tokenCheckError,
    urlDraft,
    setUrlDraft,
    cert,
  };
}

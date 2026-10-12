import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { resultCertState, type ResultCertState } from "@/lib/pipeline/resultCert";
import type { ParseSettings } from "./types";

/** The parser settings and the result-CDN certificate probe. The saved-token
 *  check is `useMineruKey`'s. */
export function useParseSettings() {
  const [settings, setSettings] = useState<ParseSettings | null>(null);
  const [error, setError] = useState<string | null>(null);

  // The endpoint as typed; empty means "no override" (`default_base_url`).
  const [urlDraft, setUrlDraft] = useState("");

  const [cert, setCert] = useState<ResultCertState | null>(null);

  // Rust owns the endpoint in force, so re-seed the field from every answer.
  const applySettings = useCallback((next: ParseSettings) => {
    setSettings(next);
    setUrlDraft(next.overridden ? next.base_url : "");
  }, []);

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
    urlDraft,
    setUrlDraft,
    cert,
  };
}

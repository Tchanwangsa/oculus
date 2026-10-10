import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getUnembeddedPdfs } from "@/lib/pipeline/retrieval";
import { useIndexStore } from "@/stores/sync/indexStore";
import type { EmbedEstimate, EmbedSettings } from "./types";

/** The settings, the outstanding-file count and the run estimate, with their loads. */
export function useEmbedSettings() {
  const [settings, setSettings] = useState<EmbedSettings | null>(null);
  const [error, setError] = useState<string | null>(null);

  // Files a run would touch — the same query the run walks, since the index
  // counts above can't give it (stale pages, partly-embedded files).
  const [outstanding, setOutstanding] = useState<number | null>(null);

  // Separate from `settings` because it is slow (Rust opens every outstanding PDF).
  const [estimate, setEstimate] = useState<EmbedEstimate | null>(null);
  const [estimating, setEstimating] = useState(false);

  const runRunning = useIndexStore((state) => state.running);
  const runResult = useIndexStore((state) => state.result);
  const runError = useIndexStore((state) => state.error);

  // One sweep at a time: each opens every outstanding PDF, so a second would
  // only repeat the work. A request during a sweep is remembered and re-run
  // after, so a changed spend limit is never quoted with a stale cut-off.
  const sweeping = useRef(false);
  const resweep = useRef(false);
  const loadEstimate = useCallback(() => {
    if (sweeping.current) {
      resweep.current = true;
      return;
    }
    sweeping.current = true;
    setEstimating(true);
    invoke<EmbedEstimate>("embed_estimate")
      .then(setEstimate)
      .catch((cause) => {
        console.error("embed estimate failed", cause);
        setEstimate(null);
      })
      .finally(() => {
        sweeping.current = false;
        setEstimating(false);
        if (resweep.current) {
          resweep.current = false;
          loadEstimate();
        }
      });
  }, []);

  const reload = useCallback(() => {
    invoke<EmbedSettings>("embed_settings")
      .then(setSettings)
      .catch((cause) => {
        console.error("embed settings failed", cause);
        setError("Could not read the embedding settings.");
      });
    getUnembeddedPdfs()
      .then((files) => setOutstanding(files.length))
      .catch(() => setOutstanding(null));
    loadEstimate();
  }, [loadEstimate]);

  useEffect(() => {
    let cancelled = false;
    invoke<EmbedSettings>("embed_settings")
      .then((next) => {
        if (!cancelled) setSettings(next);
      })
      .catch((cause) => {
        console.error("embed settings failed", cause);
        if (!cancelled) setError("Could not read the embedding settings.");
      });
    getUnembeddedPdfs()
      .then((files) => {
        if (!cancelled) setOutstanding(files.length);
      })
      .catch(() => {
        if (!cancelled) setOutstanding(null);
      });
    loadEstimate();
    return () => {
      cancelled = true;
    };
  }, [loadEstimate]);

  // A finished run moves every number here — including the detected tier the
  // estimate's hours depend on — so re-read them.
  useEffect(() => {
    if (!runRunning && (runResult || runError)) reload();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [runRunning]);

  return {
    settings,
    setSettings,
    error,
    setError,
    outstanding,
    estimate,
    estimating,
    loadEstimate,
    runRunning,
  };
}

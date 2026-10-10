import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { LocalProbe } from "./types";

/** The local MinerU server's answer; probed whenever the local engine is selected. */
export function useLocalProbe(engine: string | undefined) {
  const [probe, setProbe] = useState<LocalProbe | null>(null);
  const [probing, setProbing] = useState(false);
  const [probeError, setProbeError] = useState<string | null>(null);

  // `url` probes an endpoint that has not been saved yet; without it Rust
  // probes the one in force.
  const runProbe = useCallback(async (url?: string) => {
    setProbing(true);
    setProbeError(null);
    try {
      const candidate = url?.trim();
      const next = await invoke<LocalProbe>(
        "parse_probe_local",
        candidate ? { url: candidate } : {},
      );
      setProbe(next);
    } catch (cause) {
      console.error("MinerU server probe failed", cause);
      setProbe(null);
      setProbeError(String(cause));
    } finally {
      setProbing(false);
    }
  }, []);

  // Probe whenever local is selected; switching away drops the verdict.
  useEffect(() => {
    if (engine !== "local") {
      setProbe(null);
      setProbeError(null);
      return;
    }
    void runProbe();
  }, [engine, runProbe]);

  return { probe, probing, probeError, runProbe };
}

import { useState, useEffect } from "react";
import { appDataDir } from "@tauri-apps/api/path";

// The data dir, for building asset URLs. Asked once per app, not per caller:
// a long chat thread would otherwise make one IPC call per bubble.
let cached = "";
let inflight: Promise<string> | null = null;

function ask(): Promise<string> {
  if (!inflight) {
    inflight = appDataDir()
      .then((d) => {
        cached = d.replace(/\\/g, "/").replace(/\/$/, "");
        return cached;
      })
      .catch(() => "");
  }
  return inflight;
}

export function useDataDir() {
  const [dataDir, setDataDir] = useState(cached);

  useEffect(() => {
    if (cached) return;
    let live = true;
    ask().then((d) => {
      if (live) setDataDir(d);
    });
    return () => {
      live = false;
    };
  }, []);

  return dataDir;
}

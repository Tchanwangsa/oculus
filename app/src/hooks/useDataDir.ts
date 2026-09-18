import { useState, useEffect } from "react";
import { appDataDir } from "@tauri-apps/api/path";

/**
 * Where the library lives, for the handful of places that build an asset URL.
 *
 * Asked for **once per app**, not once per caller: the answer cannot change
 * while the app is running, and a thread of a hundred bubbles — each of which
 * may hold a picture — would otherwise be a hundred IPC calls for the same
 * string. The first caller starts the request; every later one gets it
 * synchronously.
 */
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

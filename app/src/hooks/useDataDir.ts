import { useState, useEffect } from "react";
import { appDataDir } from "@tauri-apps/api/path";

export function useDataDir() {
  const [dataDir, setDataDir] = useState("");

  useEffect(() => {
    appDataDir()
      .then((d) => setDataDir(d.replace(/\\/g, "/").replace(/\/$/, "")))
      .catch(() => {});
  }, []);

  return dataDir;
}

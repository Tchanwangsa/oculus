import type { Download } from "@/lib/browser/downloads/types";

/** A download row as Rust sends it, receiving in tab 1 unless overridden. */
export function download(id: number, over: Partial<Download> = {}): Download {
  return {
    id,
    tab: 1,
    name: `file-${id}.pdf`,
    host: "example.com",
    state: "receiving",
    received: 0,
    choice: null,
    saved_to: null,
    error: null,
    ...over,
  };
}

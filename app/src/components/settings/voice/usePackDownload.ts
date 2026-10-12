import { useCallback, useEffect, useState } from "react";
import type { UnlistenFn } from "@tauri-apps/api/event";
import type { Download } from "@/components/settings/transcription/LocalWhisper";
import type { AlignerProgress, AlignerState } from "@/lib/studio/aligner";

/** The commands behind one downloadable model: the aligner, or Kokoro's
 *  two packs, which download as one. */
export interface PackApi<S extends { status: AlignerState; bytes: number }> {
  /** Names it in the console. */
  name: string;
  status: () => Promise<S>;
  /** Resolves once it is on disk; rejects `"cancelled"` on a cancel. */
  download: () => Promise<void>;
  cancel: () => Promise<boolean>;
  remove: () => Promise<number>;
  onProgress: (handler: (progress: AlignerProgress) => void) => Promise<UnlistenFn>;
}

/**
 * A model's status, its download in flight and the last failure. Progress
 * arrives as events; a download started on an earlier visit shows at 0
 * until its next event. `api` must be stable (a module constant).
 */
export function usePackDownload<S extends { status: AlignerState; bytes: number }>(api: PackApi<S>) {
  const [status, setStatus] = useState<S | null>(null);
  const [download, setDownload] = useState<Download | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(() => {
    api
      .status()
      .then((next) => {
        setStatus(next);
        if (next.status === "downloading") {
          setDownload((d) => d ?? { received: 0, total: next.bytes });
        }
      })
      .catch((cause) => {
        console.error(`${api.name} status failed`, cause);
        setError(String(cause));
      });
  }, [api]);

  useEffect(() => {
    refresh();
    let disposed = false;
    let stop: (() => void) | undefined;
    void api
      .onProgress((p) => {
        if (p.phase === "downloading") {
          setDownload({ received: p.received, total: p.total });
          return;
        }
        setDownload(null);
        if (p.phase === "error" && p.error) setError(p.error);
        refresh();
      })
      .then((unlisten) => {
        if (disposed) unlisten();
        else stop = unlisten;
      });
    return () => {
      disposed = true;
      stop?.();
    };
  }, [api, refresh]);

  const start = () => {
    setError(null);
    setDownload({ received: 0, total: status?.bytes ?? 0 });
    api.download().catch((cause) => {
      const text = String(cause);
      setDownload(null);
      if (text !== "cancelled") setError(text);
    });
  };

  const cancel = () => {
    api.cancel().catch((cause) => {
      console.error(`${api.name} download cancel failed`, cause);
    });
  };

  /** Resolves once it is gone and the status re-read. */
  const remove = async () => {
    try {
      await api.remove();
      setError(null);
    } catch (cause) {
      console.error(`${api.name} delete failed`, cause);
      setError(String(cause));
    }
    refresh();
  };

  return { status, download, error, start, cancel, remove };
}

export type PackDownload<S extends { status: AlignerState; bytes: number }> = ReturnType<
  typeof usePackDownload<S>
>;

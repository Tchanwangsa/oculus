import { useCallback, useEffect, useState } from "react";
import type { Download } from "@/components/settings/transcription/LocalWhisper";
import {
  alignerStatus,
  cancelAlignerDownload,
  deleteAligner,
  downloadAligner,
  onAlignerProgress,
  type AlignerStatus,
} from "@/lib/studio/aligner";

/**
 * The aligner model's status, its download in flight and the last failure.
 * Progress arrives as `studio-aligner-progress` events; a download started on
 * an earlier visit shows at 0 until its next event.
 */
export function useAlignerDownload() {
  const [status, setStatus] = useState<AlignerStatus | null>(null);
  const [download, setDownload] = useState<Download | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(() => {
    alignerStatus()
      .then((next) => {
        setStatus(next);
        if (next.status === "downloading") {
          setDownload((d) => d ?? { received: 0, total: next.bytes });
        }
      })
      .catch((cause) => {
        console.error("Aligner status failed", cause);
        setError(String(cause));
      });
  }, []);

  useEffect(() => {
    refresh();
    let disposed = false;
    let stop: (() => void) | undefined;
    void onAlignerProgress((p) => {
      if (p.phase === "downloading") {
        setDownload({ received: p.received, total: p.total });
        return;
      }
      setDownload(null);
      if (p.phase === "error" && p.error) setError(p.error);
      refresh();
    }).then((unlisten) => {
      if (disposed) unlisten();
      else stop = unlisten;
    });
    return () => {
      disposed = true;
      stop?.();
    };
  }, [refresh]);

  const start = () => {
    setError(null);
    setDownload({ received: 0, total: status?.bytes ?? 0 });
    downloadAligner().catch((cause) => {
      const text = String(cause);
      setDownload(null);
      if (text !== "cancelled") setError(text);
    });
  };

  const cancel = () => {
    cancelAlignerDownload().catch((cause) => {
      console.error("Aligner download cancel failed", cause);
    });
  };

  /** Resolves once the model is gone and the status re-read. */
  const remove = async () => {
    try {
      await deleteAligner();
      setError(null);
    } catch (cause) {
      console.error("Aligner delete failed", cause);
      setError(String(cause));
    }
    refresh();
  };

  return { status, download, error, start, cancel, remove };
}

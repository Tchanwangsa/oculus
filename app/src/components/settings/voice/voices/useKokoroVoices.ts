import { useCallback, useEffect, useRef, useState } from "react";
import type { Download } from "@/components/settings/transcription/LocalWhisper";
import {
  cancelKokoroVoiceDownload,
  deleteKokoroVoice,
  downloadKokoroVoice,
  kokoroVoicesStatus,
  onKokoroVoiceProgress,
  type KokoroVoice,
} from "@/lib/studio/kokoroVoices";

/** Download all's place in its queue. */
export interface VoiceQueue {
  /** Voices finished before the one downloading. */
  done: number;
  total: number;
}

const without = <T,>(record: Record<string, T>, id: string): Record<string, T> => {
  const { [id]: _, ...rest } = record;
  return rest;
};

/**
 * Kokoro's voices with their status, each download in flight and each last
 * failure, plus Download all, which fetches the missing ones one after
 * another and stops at the first failure or cancel. Progress arrives as
 * events; a download started on an earlier visit shows at 0 until its next
 * event.
 */
export function useKokoroVoices() {
  const [voices, setVoices] = useState<KokoroVoice[] | null>(null);
  const [downloads, setDownloads] = useState<Record<string, Download>>({});
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [loadError, setLoadError] = useState<string | null>(null);
  const [queue, setQueue] = useState<VoiceQueue | null>(null);
  const stopQueue = useRef(false);

  const refresh = useCallback(() => {
    kokoroVoicesStatus()
      .then((next) => {
        setVoices(next);
        setLoadError(null);
        setDownloads((current) => {
          const merged = { ...current };
          for (const v of next) {
            if (v.status === "downloading" && !merged[v.id]) merged[v.id] = { received: 0, total: v.bytes };
          }
          return merged;
        });
      })
      .catch((cause) => {
        console.error("Kokoro voices status failed", cause);
        setLoadError(String(cause));
      });
  }, []);

  useEffect(() => {
    refresh();
    let disposed = false;
    let stop: (() => void) | undefined;
    void onKokoroVoiceProgress((p) => {
      if (p.phase === "downloading") {
        setDownloads((d) => ({ ...d, [p.id]: { received: p.received, total: p.total } }));
        return;
      }
      setDownloads((d) => without(d, p.id));
      if (p.phase === "error" && p.error) {
        const message = p.error;
        setErrors((e) => ({ ...e, [p.id]: message }));
      }
      refresh();
    }).then((unlisten) => {
      if (disposed) unlisten();
      else stop = unlisten;
    });
    return () => {
      disposed = true;
      stopQueue.current = true;
      stop?.();
    };
  }, [refresh]);

  /** Resolves true once the voice is on disk, false on a cancel or failure. */
  const fetchVoice = async (voice: KokoroVoice): Promise<boolean> => {
    setErrors((e) => without(e, voice.id));
    setDownloads((d) => ({ ...d, [voice.id]: { received: 0, total: voice.bytes } }));
    try {
      await downloadKokoroVoice(voice.id);
      return true;
    } catch (cause) {
      const text = String(cause);
      setDownloads((d) => without(d, voice.id));
      if (text !== "cancelled") setErrors((e) => ({ ...e, [voice.id]: text }));
      return false;
    }
  };

  const start = (voice: KokoroVoice) => void fetchVoice(voice);

  const cancel = (id: string) => {
    cancelKokoroVoiceDownload(id).catch((cause) => {
      console.error("Kokoro voice download cancel failed", cause);
    });
  };

  /** Resolves once the voice is gone and the status re-read. */
  const remove = async (id: string) => {
    try {
      await deleteKokoroVoice(id);
      setErrors((e) => without(e, id));
    } catch (cause) {
      console.error("Kokoro voice delete failed", cause);
      setErrors((e) => ({ ...e, [id]: String(cause) }));
    }
    refresh();
  };

  const downloadAll = async () => {
    const todo = (voices ?? []).filter((v) => v.status === "missing");
    stopQueue.current = false;
    for (const [done, voice] of todo.entries()) {
      if (stopQueue.current) break;
      setQueue({ done, total: todo.length });
      if (!(await fetchVoice(voice))) break;
    }
    setQueue(null);
    refresh();
  };

  const cancelAll = () => {
    stopQueue.current = true;
    for (const id of Object.keys(downloads)) cancel(id);
  };

  return { voices, downloads, errors, loadError, queue, start, cancel, remove, downloadAll, cancelAll };
}

export type KokoroVoices = ReturnType<typeof useKokoroVoices>;

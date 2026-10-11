import { useCallback, useSyncExternalStore } from "react";
import { onTranscribeProgress, transcribe, type TranscribeEngine } from "@/lib/lectures/transcribe";

/**
 * Transcription runs started from a player, kept outside React: a run takes
 * minutes and outlives the player that started it, so a player opened again
 * mid-run finds it here. Keyed by the path exactly as passed to `transcribe`,
 * which is how Rust's progress events name it.
 */

/** Window event (detail: `{ path, vtt }`): a run's transcript is on disk and
 *  its `after` step has run. */
export const TRANSCRIBED_EVENT = "oculus:transcribed";

export interface TranscribedDetail {
  path: string;
  /** Absolute path of the `.vtt` written beside the video. */
  vtt: string;
}

export interface TranscriptionRun {
  /** `done`: the VTT is written and `after` has run, but the player has not
   *  loaded the cues yet — still a run, so Transcribe stays hidden. */
  phase: "extracting" | "transcribing" | "done" | "error";
  /** From 1 while transcribing; 0 while extracting. */
  chunk: number;
  chunks: number;
  /** The engine at work while transcribing; it changes on a fall-through. */
  engine?: TranscribeEngine;
  /** Only on `error`: the sentence `transcribe` rejected with. */
  error?: string;
}

const runs = new Map<string, TranscriptionRun>();
const listeners = new Set<() => void>();
let listening = false;

function set(path: string, run: TranscriptionRun | null) {
  if (run) runs.set(path, run);
  else runs.delete(path);
  for (const fn of listeners) fn();
}

function subscribe(fn: () => void): () => void {
  listeners.add(fn);
  return () => listeners.delete(fn);
}

/** One app-wide progress listener, for runs this window started. */
function listen() {
  if (listening) return;
  listening = true;
  void onTranscribeProgress((p) => {
    const run = runs.get(p.path);
    if (!run || run.phase === "error" || run.phase === "done") return;
    if (p.phase === "extracting" || p.phase === "transcribing") {
      set(p.path, { phase: p.phase, chunk: p.chunk, chunks: p.chunks, engine: p.engine });
    }
  });
}

/**
 * Transcribe `path`, then run `after` with the VTT's path (a lecture records
 * it in the DB). Both finish even if the player that asked has unmounted;
 * `TRANSCRIBED_EVENT` says so, and the run stays `done` until a player has
 * read the cues (`settleTranscription`). A failure stays until retried.
 */
export function startTranscription(path: string, after?: (vtt: string) => Promise<void>) {
  const current = runs.get(path);
  if (current && current.phase !== "error") return;
  listen();
  set(path, { phase: "extracting", chunk: 0, chunks: 0 });
  transcribe(path)
    .then(async (vtt) => {
      await after?.(vtt);
      set(path, { phase: "done", chunk: 0, chunks: 0 });
      window.dispatchEvent(
        new CustomEvent<TranscribedDetail>(TRANSCRIBED_EVENT, { detail: { path, vtt } }),
      );
    })
    .catch((e) => set(path, { phase: "error", chunk: 0, chunks: 0, error: String(e) }));
}

/** A player has read `path`'s transcript (or failed to): a `done` run ends.
 *  Runs still in flight or failed are left alone. */
export function settleTranscription(path: string) {
  if (runs.get(path)?.phase === "done") set(path, null);
}

/** The run for `path`, if one is in flight or failed, and how to start one. */
export function useTranscription(
  path: string | null,
  after?: (vtt: string) => Promise<void>,
) {
  const run = useSyncExternalStore(subscribe, () => (path ? (runs.get(path) ?? null) : null));
  const start = useCallback(() => {
    if (path) startTranscription(path, after);
  }, [path, after]);
  return { run, start };
}

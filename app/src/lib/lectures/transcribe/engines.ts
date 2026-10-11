import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

/** `ENGINES` in `app/src-tauri/src/transcribe/engine.rs`. */
export type TranscribeEngine = "groq" | "whisper" | "apple";

/** The order a run tries the engines in until Settings changes it. */
export const DEFAULT_ENGINE_ORDER: readonly TranscribeEngine[] = ["groq", "whisper", "apple"];

/** As Rust's `engine_label` names each engine in its messages. */
export const ENGINE_LABELS: Record<TranscribeEngine, string> = {
  groq: "Groq",
  apple: "on-device speech",
  whisper: "local Whisper",
};

/** Mirrors `Progress` in `app/src-tauri/src/transcribe/app.rs`. */
export interface TranscribeProgress {
  /** The path exactly as passed to `transcribe`. */
  path: string;
  phase: "extracting" | "transcribing" | "complete" | "error";
  /** The engine at work, on `transcribing` and `complete`. */
  engine?: TranscribeEngine;
  /** From 1 while transcribing; 0 while extracting. */
  chunk: number;
  chunks: number;
  /** Only on `error`; the same text the call rejects with. */
  error?: string;
}

/**
 * Transcribe a video inside the library — absolute, or relative to the data
 * directory — and resolve to the absolute path of the `.vtt` written beside
 * it. Rejects with a sentence fit to show: no key, Groq's rate limit and when
 * to retry, or what failed. Nothing is written on failure.
 */
export function transcribe(path: string): Promise<string> {
  return invoke<string>("transcribe_video", { path });
}

export function onTranscribeProgress(
  handler: (progress: TranscribeProgress) => void,
): Promise<UnlistenFn> {
  return listen<TranscribeProgress>("transcribe-progress", (event) => handler(event.payload));
}

export function hasGroqKey(): Promise<boolean> {
  return invoke<boolean>("groq_has_api_key");
}

/**
 * Rust checks the key against Groq's free model list before keeping it.
 * `"unverified"`: Groq was unreachable or rate-limited, and it was kept anyway.
 */
export function saveGroqKey(key: string): Promise<"ok" | "unverified"> {
  return invoke<"ok" | "unverified">("groq_set_api_key", { key });
}

export function deleteGroqKey(): Promise<void> {
  return invoke<void>("groq_delete_api_key");
}

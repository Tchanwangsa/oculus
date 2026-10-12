import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

// The narration aligner's model (wav2vec2, ONNX), downloaded to the app's
// data folder (`app/src-tauri/src/studio/commands/aligner.rs`). The status is
// local; only a download touches the network.

/** Mirrors `timing::Status`. */
export type AlignerState = "missing" | "downloading" | "present";

/** Mirrors `AlignerStatus`. */
export interface AlignerStatus {
  status: AlignerState;
  /** The download's size. */
  bytes: number;
}

export function alignerStatus(): Promise<AlignerStatus> {
  return invoke<AlignerStatus>("studio_aligner_status");
}

/** Resolves once the model is on disk; rejects `"cancelled"` on a cancel,
 *  else with a sentence fit to show. */
export function downloadAligner(): Promise<void> {
  return invoke<void>("studio_aligner_download");
}

/** false when no download is running. */
export function cancelAlignerDownload(): Promise<boolean> {
  return invoke<boolean>("studio_aligner_cancel");
}

/** Resolves to the bytes freed. */
export function deleteAligner(): Promise<number> {
  return invoke<number>("studio_aligner_delete");
}

/** Mirrors `Progress`; throttled to about one every 150 ms. */
export interface AlignerProgress {
  phase: "downloading" | "complete" | "cancelled" | "error";
  received: number;
  total: number;
  /** Only on `error`. */
  error?: string;
}

export function onAlignerProgress(handler: (progress: AlignerProgress) => void): Promise<UnlistenFn> {
  return listen<AlignerProgress>("studio-aligner-progress", (event) => handler(event.payload));
}

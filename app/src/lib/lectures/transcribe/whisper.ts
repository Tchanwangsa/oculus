import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

// whisper.cpp on this Mac with a model downloaded from Hugging Face
// (`app/src-tauri/src/transcribe/engines/whisper.rs`). Listing the models is local;
// only a download touches the network.

/** Mirrors `Fit` in `app/src-tauri/src/transcribe/engines/whisper_models/models.rs`. */
export type WhisperFit = "recommended" | "ok" | "too_large";

export interface WhisperModel {
  id: string;
  label: string;
  /** The file's size. */
  bytes: number;
  /** Roughly what a run holds in memory. */
  ramBytes: number;
  downloaded: boolean;
  /** A download is in flight in the app, perhaps started by an earlier visit. */
  downloading: boolean;
  fit: WhisperFit;
}

/** Mirrors `WhisperListing` (a `Catalogue` and the helper);
 *  `models` runs worst to best transcription. */
export interface WhisperCatalogue {
  /** `whisper-cli` is found; without it no model runs. */
  helper: boolean;
  /** null when the RAM size could not be read. */
  totalRamBytes: number | null;
  gpu: boolean;
  vadDownloaded: boolean;
  /** The model a run uses when none is chosen and it is downloaded. */
  defaultModel: string;
  models: WhisperModel[];
}

export function whisperModels(): Promise<WhisperCatalogue> {
  return invoke<WhisperCatalogue>("whisper_models");
}

/** Resolves once the model is on disk; rejects `"cancelled"` on a cancel,
 *  else with a sentence fit to show. */
export function downloadWhisperModel(id: string): Promise<void> {
  return invoke<void>("whisper_download_model", { id });
}

/** false when no download of `id` is running. */
export function cancelWhisperDownload(id: string): Promise<boolean> {
  return invoke<boolean>("whisper_cancel_download", { id });
}

/** Resolves to the bytes freed. */
export function deleteWhisperModel(id: string): Promise<number> {
  return invoke<number>("whisper_delete_model", { id });
}

/** Mirrors `ModelProgress`; throttled to about one every 150 ms. */
export interface WhisperModelProgress {
  id: string;
  phase: "downloading" | "complete" | "cancelled" | "error";
  /** Bytes of the model file. */
  received: number;
  total: number;
  /** Only on `error`. */
  error?: string;
}

export function onWhisperModelProgress(
  handler: (progress: WhisperModelProgress) => void,
): Promise<UnlistenFn> {
  return listen<WhisperModelProgress>("whisper-model-progress", (event) => handler(event.payload));
}

/**
 * The model a run would use, as Rust's `pick` chooses it: the chosen one when
 * set (null if it is not downloaded — the run would fail), else the default if
 * downloaded, else the last downloaded in catalogue order.
 */
export function pickWhisperModel(
  catalogue: Pick<WhisperCatalogue, "models" | "defaultModel">,
  chosen: string | null,
): WhisperModel | null {
  const onDisk = catalogue.models.filter((m) => m.downloaded);
  if (chosen) return onDisk.find((m) => m.id === chosen) ?? null;
  return onDisk.find((m) => m.id === catalogue.defaultModel) ?? onDisk[onDisk.length - 1] ?? null;
}

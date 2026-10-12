import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { AlignerProgress, AlignerState } from "@/lib/studio/aligner";

// Kokoro's 20 US English voices, one small file each, downloaded one at a
// time (`app/src-tauri/src/studio/commands/voice_files.rs`). The status is
// local; only a download touches the network.

/** Mirrors `VoiceRow`. */
export interface KokoroVoice {
  /** `af_heart`: what the `voice` setting takes. */
  id: string;
  /** `Heart`. */
  name: string;
  accent: string;
  /** `female` or `male`. */
  gender: string;
  /** Kokoro's published grade of the voice's training data, `A` to `F`. */
  grade: string;
  bytes: number;
  status: AlignerState;
}

/** What a fresh install suggests, with the packs. */
export const STARTER_VOICES: readonly string[] = ["af_heart", "am_michael"];

export function kokoroVoicesStatus(): Promise<KokoroVoice[]> {
  return invoke<KokoroVoice[]>("studio_voices_status");
}

/** Resolves once the voice is on disk; rejects `"cancelled"` on a cancel,
 *  else with a sentence fit to show. */
export function downloadKokoroVoice(id: string): Promise<void> {
  return invoke<void>("studio_voice_download", { id });
}

/** false when that voice is not downloading. */
export function cancelKokoroVoiceDownload(id: string): Promise<boolean> {
  return invoke<boolean>("studio_voice_cancel", { id });
}

/** Resolves to the bytes freed. */
export function deleteKokoroVoice(id: string): Promise<number> {
  return invoke<number>("studio_voice_delete", { id });
}

/** The aligner's payload, naming the voice it is about. */
export type KokoroVoiceProgress = AlignerProgress & { id: string };

export function onKokoroVoiceProgress(
  handler: (progress: KokoroVoiceProgress) => void,
): Promise<UnlistenFn> {
  return listen<KokoroVoiceProgress>("studio-voice-file-progress", (event) => handler(event.payload));
}

/** `Heart · American, female`. */
export function kokoroVoiceLabel(voice: KokoroVoice): string {
  return `${voice.name} · ${voice.accent}, ${voice.gender}`;
}

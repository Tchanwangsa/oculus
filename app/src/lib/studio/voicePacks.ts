import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { AlignerProgress, AlignerState } from "@/lib/studio/aligner";

// Kokoro's two packs (the model and voices, and the G2P), downloaded together
// to the app's data folder, and a sample clip synthesised on this Mac
// (`app/src-tauri/src/studio/commands/kokoro.rs`). The status is local; only
// a download touches the network, and the sample is never a billed call.

/** Mirrors `PackRow`. */
export interface VoicePack {
  id: "voice-kokoro" | "g2p-en-us";
  status: AlignerState;
  bytes: number;
}

/** Mirrors `VoiceFile`. */
export interface KokoroVoiceFile {
  id: string;
  downloaded: boolean;
}

/** Mirrors `VoicePacks`. */
export interface VoicePacksStatus {
  /** Both packs together: present only when both are. */
  status: AlignerState;
  /** Both packs' download size. */
  bytes: number;
  packs: VoicePack[];
  /** The pack's voices, downloaded or not, then any other voice file. */
  voices: KokoroVoiceFile[];
}

export function voicePacksStatus(): Promise<VoicePacksStatus> {
  return invoke<VoicePacksStatus>("studio_voice_packs_status");
}

/** Resolves once both packs are on disk; rejects `"cancelled"` on a cancel,
 *  else with a sentence fit to show. */
export function downloadVoicePacks(): Promise<void> {
  return invoke<void>("studio_voice_packs_download");
}

/** false when no download is running. */
export function cancelVoicePacksDownload(): Promise<boolean> {
  return invoke<boolean>("studio_voice_packs_cancel");
}

/** Resolves to the bytes freed. */
export function deleteVoicePacks(): Promise<number> {
  return invoke<number>("studio_voice_packs_delete");
}

/** The same payload as the aligner's, over both packs. */
export type VoicePacksProgress = AlignerProgress;

export function onVoicePacksProgress(
  handler: (progress: VoicePacksProgress) => void,
): Promise<UnlistenFn> {
  return listen<VoicePacksProgress>("studio-voice-progress", (event) => handler(event.payload));
}

/** A fixed sentence in `voice` at `speed`, as WAV bytes; rejects with a
 *  sentence fit to show (a missing pack, a voice not on disk). */
export function voiceSample(voice: string, speed: number): Promise<ArrayBuffer> {
  return invoke<ArrayBuffer>("studio_voice_sample", { voice, speed, text: null });
}

const ACCENTS: Record<string, string> = { a: "American", b: "British" };
const GENDERS: Record<string, string> = { f: "female", m: "male" };

/** `af_heart` → `Heart`, with `American, female` from Kokoro's prefix when
 *  it is one Kokoro uses; any other id is its own name. */
export function kokoroVoiceName(id: string): { name: string; detail: string | null } {
  const match = /^([a-z])([fm])_([a-z0-9_]+)$/.exec(id);
  if (!match) return { name: id, detail: null };
  const [, accent, gender, rest] = match;
  const name = rest
    .split("_")
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
  const detail = [ACCENTS[accent], GENDERS[gender]].filter(Boolean).join(", ");
  return { name, detail: detail || null };
}

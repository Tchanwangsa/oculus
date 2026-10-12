import { invoke } from "@tauri-apps/api/core";

// The `voice` settings row Studio narrates with, read and written by Rust
// (`app/src-tauri/src/studio/commands/voice.rs`). Keyless OpenAI-compatible
// TTS servers only. Nothing here synthesises: listing models is free.

/** Mirrors `VoiceRow`; an unset field is null. */
export interface VoiceSettings {
  /** Normalised: no trailing slash or `/v1`. */
  baseUrl: string | null;
  model: string | null;
  voice: string | null;
  /** Positive; 1.0 when unset. */
  speed: number;
}

/** The slider's range; Rust accepts any positive speed. */
export const SPEED_MIN = 0.5;
export const SPEED_MAX = 2;
export const SPEED_STEP = 0.05;

export function readVoiceSettings(): Promise<VoiceSettings> {
  return invoke<VoiceSettings>("studio_voice_settings");
}

/** Writes the whole row and resolves to what was stored; rejects with a
 *  sentence fit to show (a bad URL, a non-positive speed). */
export function writeVoiceSettings(settings: VoiceSettings): Promise<VoiceSettings> {
  return invoke<VoiceSettings>("studio_set_voice_settings", { settings });
}

/** The model ids `GET {baseUrl}/v1/models` lists; rejects when the server
 *  is unreachable or does not list models. */
export function listVoiceModels(baseUrl: string): Promise<string[]> {
  return invoke<string[]>("studio_voice_models", { baseUrl });
}

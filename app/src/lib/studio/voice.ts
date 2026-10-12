import { invoke } from "@tauri-apps/api/core";

// The `voice` settings row Studio narrates with, read and written by Rust
// (`app/src-tauri/src/studio/commands/voice.rs`): the engine (Kokoro in the
// app, or a keyless OpenAI-compatible TTS server), the voice and speed, and
// the server's URL and model. Nothing here synthesises: listing models is free.

/** `server` asks a TTS server; `kokoro` runs Kokoro inside the app. */
export type VoiceEngine = "server" | "kokoro";

/** Mirrors `VoiceRow`; an unset field is null. */
export interface VoiceSettings {
  /** No default: narration fails until one is chosen. A stored row with a
   *  URL and no engine reads as `server`. */
  engine: VoiceEngine | null;
  /** The server engine's only; normalised: no trailing slash or `/v1`. */
  baseUrl: string | null;
  /** The server engine's only. */
  model: string | null;
  voice: string | null;
  /** Positive; `DEFAULT_SPEED` when unset. */
  speed: number;
}

/** Rust's `voice::DEFAULT_SPEED`: a little under the voice's own pace. */
export const DEFAULT_SPEED = 0.9;

/** The slider's range; Rust accepts any positive speed. */
export const SPEED_MIN = 0.5;
export const SPEED_MAX = 2;
export const SPEED_STEP = 0.05;

/** Where the voice is set up; the style chip and the New video error link here. */
export const VOICE_SETTINGS_HREF = "/settings/voice";

/**
 * The sentence for what the saved settings still lack before a video can be
 * narrated, or null when they are enough: an engine and a voice, and for the
 * server engine also its URL and model. A thin mirror of Rust's
 * `voice::resolve` (`app/src-tauri/src/studio/voice/mod.rs`), which stays the
 * source of truth: it also reads a video's own voice block and the CLI's
 * flags, and checks a Kokoro voice id, none of which a new video has yet.
 */
export function voiceProblem(settings: VoiceSettings): string | null {
  const set = (v: string | null) => (v ?? "").trim() !== "";
  if (!settings.engine) return "Choose a narration engine in Settings → Voice before making a video.";
  if (settings.engine === "server") {
    if (!set(settings.baseUrl)) return "Set a TTS server in Settings → Voice before making a video.";
    if (!set(settings.model)) return "Set a TTS model in Settings → Voice before making a video.";
  }
  if (!set(settings.voice)) return "Choose a narration voice in Settings → Voice before making a video.";
  return null;
}

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

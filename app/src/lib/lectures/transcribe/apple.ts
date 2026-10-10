import { invoke } from "@tauri-apps/api/core";

// macOS 26's speech recogniser. Settings only reads its status; downloading a
// language happens at transcription time.

/** Mirrors the `apple_speech_status` command's answer. Locale ids are `en_AU`. */
export interface AppleSpeechStatus {
  available: boolean;
  /** Why it is unavailable, in a sentence fit to show; null when available. */
  reason: string | null;
  supported: string[];
  /** Locales whose model is already on this Mac. */
  installed: string[];
  defaultLocale: string | null;
}

/** Resolves `available: false` with a reason rather than rejecting when the
 *  recogniser is missing; rejects only on an unexpected failure. */
export function appleSpeechStatus(): Promise<AppleSpeechStatus> {
  return invoke<AppleSpeechStatus>("apple_speech_status");
}

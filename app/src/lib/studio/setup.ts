import { alignerStatus, type AlignerStatus } from "@/lib/studio/aligner";
import { NARRATION_SETTINGS_HREF, readVoiceSettings, voiceProblem, type VoiceSettings } from "@/lib/studio/voice";
import { kokoroVoicesStatus, type KokoroVoice } from "@/lib/studio/kokoroVoices";
import { voicePacksStatus, type VoicePacksStatus } from "@/lib/studio/voicePacks";

// What a video needs before it can be narrated: a configured voice, Kokoro's
// model, pronunciation pack and chosen voice when Kokoro is the engine, and
// the word-timing model. Reading it is local and free; nothing here
// downloads.

export const WORD_TIMING_SETTINGS_HREF = "/settings/word-timing";

/** One missing piece and the Settings page that fixes it. */
export interface SetupProblem {
  id: "narration" | "packs" | "voice" | "timing";
  message: string;
  href: string;
}

const mb = (bytes: number) => `${Math.max(1, Math.round(bytes / 1_000_000))} MB`;

/**
 * Everything missing, narration first. `packs` and `voices` matter only to
 * the Kokoro engine, and a download that is still running counts as not there
 * yet.
 * Rust's narrate refuses the same cases (`voice::resolve`, the pack and
 * aligner checks), so this only says so before a folder is made.
 */
export function setupProblems(
  voice: VoiceSettings,
  packs: VoicePacksStatus,
  aligner: AlignerStatus,
  voices: KokoroVoice[],
): SetupProblem[] {
  const problems: SetupProblem[] = [];
  const narration = voiceProblem(voice);
  if (narration) problems.push({ id: "narration", message: narration, href: NARRATION_SETTINGS_HREF });
  if (voice.engine === "kokoro" && packs.status !== "present") {
    problems.push({
      id: "packs",
      message:
        packs.status === "downloading"
          ? "Kokoro's model and pronunciation pack are still downloading."
          : `Download Kokoro's model and pronunciation pack (${mb(packs.bytes)}) before making a video.`,
      href: NARRATION_SETTINGS_HREF,
    });
  }
  const chosen = voices.find((v) => v.id === voice.voice);
  if (voice.engine === "kokoro" && !narration && chosen?.status !== "present") {
    problems.push({
      id: "voice",
      message: chosen
        ? chosen.status === "downloading"
          ? `The ${chosen.name} voice is still downloading.`
          : `Download the ${chosen.name} voice before making a video.`
        : `${voice.voice} is not a Kokoro voice you can download: choose another before making a video.`,
      href: NARRATION_SETTINGS_HREF,
    });
  }
  if (aligner.status !== "present") {
    problems.push({
      id: "timing",
      message:
        aligner.status === "downloading"
          ? "The word-timing model is still downloading."
          : `Download the word-timing model (${mb(aligner.bytes)}) before making a video.`,
      href: WORD_TIMING_SETTINGS_HREF,
    });
  }
  return problems;
}

/** Reads the saved voice and the models' and voices' status; rejects with a
 *  sentence when one cannot be read. */
export async function readSetupProblems(): Promise<{ voice: VoiceSettings; problems: SetupProblem[] }> {
  const [voice, packs, aligner, voices] = await Promise.all([
    readVoiceSettings(),
    voicePacksStatus(),
    alignerStatus(),
    kokoroVoicesStatus(),
  ]);
  return { voice, problems: setupProblems(voice, packs, aligner, voices) };
}

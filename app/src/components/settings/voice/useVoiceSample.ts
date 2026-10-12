import { voiceSample } from "@/lib/studio/voicePacks";
import { useClipPlayer } from "./useClipPlayer";

/**
 * Kokoro's sample clips (`studio_voice_sample`) through `useClipPlayer`,
 * keyed by voice id so one row plays at a time. Each play makes the clip
 * afresh, so it follows the speed as it is now.
 */
export function useVoiceSample() {
  const player = useClipPlayer();
  return {
    phaseOf: player.phaseOf,
    error: player.error,
    play: (voice: string, speed: number) => player.play(voice, () => voiceSample(voice, speed)),
    stop: player.stop,
  };
}

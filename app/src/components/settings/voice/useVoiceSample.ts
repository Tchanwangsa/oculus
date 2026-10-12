import { voiceSample } from "@/lib/studio/voicePacks";
import { useClipPlayer } from "./useClipPlayer";

/**
 * Kokoro's sample clip (`studio_voice_sample`) through `useClipPlayer`. Each
 * play makes the clip afresh, so it follows the voice and speed as they are
 * now.
 */
export function useVoiceSample() {
  const player = useClipPlayer();
  return {
    phase: player.phaseOf("sample"),
    error: player.error?.message ?? null,
    play: (voice: string, speed: number) => player.play("sample", () => voiceSample(voice, speed)),
    stop: player.stop,
  };
}

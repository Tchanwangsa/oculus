import { Play, Stop } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { SettingRow } from "./SettingRow";
import { useVoiceSample } from "./useVoiceSample";

/**
 * Plays a fixed sentence in Kokoro's chosen voice at the chosen speed, made
 * on this Mac: never a billed call. Disabled until both packs and a voice
 * are there; while the clip is being made there is nothing to stop.
 */
export function VoiceSampleRow({ voice, speed, ready }: {
  voice: string | null;
  speed: number;
  /** Both packs are downloaded. */
  ready: boolean;
}) {
  const { phase, error, play, stop } = useVoiceSample();

  return (
    <SettingRow
      label="Sample"
      hint={
        !ready
          ? "Download the packs to hear a voice."
          : voice
            ? "A sentence in this voice at this speed, made on this Mac."
            : "Choose a voice to hear it."
      }
      error={error}
    >
      {phase === "playing" ? (
        <Button variant="outline" size="xs" className="w-[5.5rem]" onClick={stop}>
          <Stop size={12} weight="fill" />
          Stop
        </Button>
      ) : (
        <Button
          variant="outline"
          size="xs"
          className="w-[5.5rem]"
          disabled={!ready || !voice || phase === "making"}
          onClick={() => voice && void play(voice, speed)}
        >
          <Play size={12} weight="fill" />
          {phase === "making" ? "Making…" : "Play"}
        </Button>
      )}
    </SettingRow>
  );
}

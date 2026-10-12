import { useCallback, useEffect, useRef, useState } from "react";
import { voiceSample } from "@/lib/studio/voicePacks";

export type SamplePhase = "idle" | "making" | "playing";

/**
 * Kokoro's sample clip: made on this Mac (`studio_voice_sample`), played from
 * a blob URL, stopped on demand or when the page goes. Each play makes the
 * clip afresh, so it follows the voice and speed as they are now.
 */
export function useVoiceSample() {
  const [phase, setPhase] = useState<SamplePhase>("idle");
  const [error, setError] = useState<string | null>(null);
  const audio = useRef<HTMLAudioElement | null>(null);
  const url = useRef<string | null>(null);
  // A play that a stop or a newer play has overtaken drops its clip.
  const seq = useRef(0);

  const release = useCallback(() => {
    audio.current?.pause();
    audio.current = null;
    if (url.current) URL.revokeObjectURL(url.current);
    url.current = null;
  }, []);

  useEffect(() => () => {
    seq.current += 1;
    release();
  }, [release]);

  const stop = () => {
    seq.current += 1;
    release();
    setPhase("idle");
  };

  const play = async (voice: string, speed: number) => {
    const mine = ++seq.current;
    release();
    setError(null);
    setPhase("making");
    try {
      const wav = await voiceSample(voice, speed);
      if (seq.current !== mine) return;
      url.current = URL.createObjectURL(new Blob([wav], { type: "audio/wav" }));
      const player = new Audio(url.current);
      audio.current = player;
      player.onended = () => {
        if (seq.current !== mine) return;
        release();
        setPhase("idle");
      };
      await player.play();
      if (seq.current === mine) setPhase("playing");
    } catch (cause) {
      if (seq.current !== mine) return;
      console.error("Voice sample failed", cause);
      release();
      setError(String(cause));
      setPhase("idle");
    }
  };

  return { phase, error, play, stop };
}

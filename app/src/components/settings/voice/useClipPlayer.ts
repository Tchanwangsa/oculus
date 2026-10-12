import { useCallback, useEffect, useRef, useState } from "react";

export type SamplePhase = "idle" | "making" | "playing";

/**
 * One WAV clip at a time, made on request (Kokoro on this computer) and
 * played from a blob URL. Each clip has a key: playing another stops the
 * first, and a stop or a newer play drops a clip still being made. The URL
 * is revoked when the clip ends, stops, or the page goes.
 */
export function useClipPlayer() {
  const [current, setCurrent] = useState<{ key: string; phase: SamplePhase } | null>(null);
  const [error, setError] = useState<{ key: string; message: string } | null>(null);
  const audio = useRef<HTMLAudioElement | null>(null);
  const url = useRef<string | null>(null);
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

  const stop = useCallback(() => {
    seq.current += 1;
    release();
    setCurrent(null);
  }, [release]);

  const play = useCallback(async (key: string, make: () => Promise<ArrayBuffer>) => {
    const mine = ++seq.current;
    release();
    setError(null);
    setCurrent({ key, phase: "making" });
    try {
      const wav = await make();
      if (seq.current !== mine) return;
      url.current = URL.createObjectURL(new Blob([wav], { type: "audio/wav" }));
      const player = new Audio(url.current);
      audio.current = player;
      player.onended = () => {
        if (seq.current !== mine) return;
        release();
        setCurrent(null);
      };
      await player.play();
      if (seq.current === mine) setCurrent({ key, phase: "playing" });
    } catch (cause) {
      if (seq.current !== mine) return;
      console.error("Clip failed", cause);
      release();
      setError({ key, message: String(cause) });
      setCurrent(null);
    }
  }, [release]);

  /** `key`'s phase: only the current clip is anything but idle. */
  const phaseOf = (key: string): SamplePhase => (current?.key === key ? current.phase : "idle");

  return { current, error, play, stop, phaseOf };
}

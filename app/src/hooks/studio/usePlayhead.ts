import { useCallback, useEffect, useRef, useState } from "react";

/** Seconds between ticks while playing: about 20 frames a second, which the
 *  stage keeps up with by skipping the ones it is too late for. */
const TICK = 0.05;

/** Where `t` may be set: from 0 up to, not including, the duration. */
const roundT = (t: number, duration: number) => Math.min(Math.max(0, Math.round(t * 100) / 100), duration - 0.01);

export interface Playhead {
  t: number;
  playing: boolean;
  /** Moves the playhead and stops playing (a scrub). */
  seek: (t: number) => void;
  toggle: () => void;
  pause: () => void;
}

/** A looping playhead over `[0, duration)`. */
export function usePlayhead(duration: number): Playhead {
  const [t, setT] = useState(0);
  const [playing, setPlaying] = useState(false);
  const at = useRef(0);
  at.current = t;

  // A shorter duration can leave the playhead past its end.
  useEffect(() => {
    setT((cur) => (cur >= duration ? 0 : cur));
  }, [duration]);

  useEffect(() => {
    if (!playing) return;
    const origin = performance.now() - at.current * 1000;
    let last = 0;
    let frame = 0;
    const tick = (now: number) => {
      if (now - last >= TICK * 1000) {
        last = now;
        setT(roundT(((now - origin) / 1000) % duration, duration));
      }
      frame = requestAnimationFrame(tick);
    };
    frame = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(frame);
  }, [playing, duration]);

  const seek = useCallback(
    (next: number) => {
      setPlaying(false);
      setT(roundT(next, duration));
    },
    [duration],
  );
  const toggle = useCallback(() => setPlaying((p) => !p), []);
  const pause = useCallback(() => setPlaying(false), []);
  return { t, playing, seek, toggle, pause };
}

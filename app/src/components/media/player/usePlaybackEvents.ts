import { useMemo, useRef, useState, type RefObject } from "react";
import { spanAt, type Cue } from "@/lib/lectures/media";

/** The clock: the playhead, the active cue and the file's own length, with the
 *  element-event handlers that keep them current. */
export function usePlaybackEvents(
  cues: Cue[],
  videoRef: RefObject<HTMLVideoElement | null>,
) {
  const cueStarts = useMemo(() => cues.map((cue) => cue.start), [cues]);
  const [activeCueIdx, setActiveCueIdx] = useState(-1);
  const [currentTime, setCurrentTime] = useState(0);
  /** The playhead for consumers that must not re-render with it: a prop would
   *  break a memoised dock on every `timeupdate`. */
  const atRef = useRef(0);
  /** The file's own length — a catalogue duration can run short of the
   *  recording. `fallbackDuration` stands in until metadata arrives. */
  const [fileDuration, setFileDuration] = useState(0);

  /** Where a paused seek is headed until it lands: the clock shows this, not
   *  the keyframe a `fastSeek` stops at on the way. */
  const targetRef = useRef<number | null>(null);
  /** The element with a seek in flight; another element starts afresh. */
  const busyRef = useRef<HTMLVideoElement | null>(null);
  const queuedRef = useRef(false);

  /** One seek at a time, newest target wins: a keyframe `fastSeek` first, then
   *  the exact time. Paused, the app's video kept its old picture under exact
   *  seeks while the scrub preview, which seeks this way, repainted. */
  const issue = (v: HTMLVideoElement, exact: boolean) => {
    const t = targetRef.current;
    if (t === null) return;
    busyRef.current = v;
    queuedRef.current = false;
    v.addEventListener("seeked", () => landed(v, exact), { once: true });
    if (!exact && typeof v.fastSeek === "function") v.fastSeek(t);
    else v.currentTime = t;
  };

  const landed = (v: HTMLVideoElement, exact: boolean) => {
    if (busyRef.current !== v) return;
    const t = targetRef.current;
    if (queuedRef.current) return issue(v, false);
    if (!exact && t !== null && Math.abs(v.currentTime - t) > 0.05) return issue(v, true);
    busyRef.current = null;
    targetRef.current = null;
    handleTimeUpdate();
  };

  /** Seek the element. Playing, it simply jumps; paused, see `issue`. */
  const seekTo = (seconds: number) => {
    const v = videoRef.current;
    if (!v) return;
    if (!v.paused || v.readyState < 1) {
      targetRef.current = null;
      busyRef.current = null;
      v.currentTime = seconds;
      return;
    }
    targetRef.current = seconds;
    queuedRef.current = true;
    if (busyRef.current !== v) issue(v, false);
  };

  const handleTimeUpdate = () => {
    const v = videoRef.current;
    if (!v) return;
    const t = targetRef.current ?? v.currentTime;
    setCurrentTime(t);
    atRef.current = t;

    setActiveCueIdx(spanAt(cueStarts, t));
  };

  // A stream still being sized reports Infinity or NaN; keep the fallback.
  const handleLoadedMetadata = () => {
    const v = videoRef.current;
    if (!v) return;
    if (Number.isFinite(v.duration) && v.duration > 0) setFileDuration(v.duration);
  };

  return {
    activeCueIdx,
    setActiveCueIdx,
    currentTime,
    setCurrentTime,
    atRef,
    fileDuration,
    setFileDuration,
    handleTimeUpdate,
    handleLoadedMetadata,
    seekTo,
  };
}

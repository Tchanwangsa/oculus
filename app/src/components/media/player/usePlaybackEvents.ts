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

  const handleTimeUpdate = () => {
    const v = videoRef.current;
    if (!v) return;
    const t = v.currentTime;
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
  };
}

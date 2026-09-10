import { useCallback, useEffect, useRef, useState } from "react";
import { cn } from "@/lib/utils";
import { fmtTime } from "@/lib/lectures";

/** Thumbnail width; the height follows the recording's own aspect ratio. */
const THUMB_W = 168;

interface ScrubPreviewProps {
  /** Same URL the player streams. `null` before the video is downloaded — the
   *  time readout still appears, there is just no frame to show. */
  src: string | null;
  /** Time under the pointer, seconds. */
  time: number;
  /** Pointer offset inside the scrub bar, px. */
  x: number;
  /** Scrub bar width, px — the box is clamped to stay inside it. */
  trackWidth: number;
  /** Match the control bar's readout on an over-an-hour recording. */
  forceHours?: boolean;
  visible: boolean;
}

/**
 * The frame under the pointer while hovering or dragging the scrub bar, the
 * way YouTube previews a seek before you commit to it.
 *
 * It is a second `<video>` on the same localhost source rather than a
 * pre-rendered sprite sheet: the file is already on disk and the media server
 * serves ranges, so seeking a muted decoder is cheaper than generating and
 * storing a storyboard for every recording at download time.
 *
 * Seeks are gated one at a time. A pointer sweep across the bar fires a move
 * event per frame, and assigning `currentTime` mid-seek makes WebKit drop the
 * earlier target — so a move while a seek is in flight only parks its time in
 * `pendingRef`, and `seeked` starts the next one. The preview lands on the
 * last position the pointer visited instead of a random one along the way.
 */
export function ScrubPreview({
  src,
  time,
  x,
  trackWidth,
  forceHours,
  visible,
}: ScrubPreviewProps) {
  const videoRef = useRef<HTMLVideoElement>(null);
  const pendingRef = useRef<number | null>(null);
  const seekingRef = useRef(false);
  /** A frame has actually been decoded — until then the box is empty black. */
  const [ready, setReady] = useState(false);
  const [failed, setFailed] = useState(false);
  const [aspect, setAspect] = useState(16 / 9);

  const flush = useCallback(() => {
    const v = videoRef.current;
    if (!v || seekingRef.current || v.readyState < 1) return;
    const t = pendingRef.current;
    if (t === null) return;
    pendingRef.current = null;
    if (Math.abs(v.currentTime - t) < 0.05) return;
    seekingRef.current = true;
    // fastSeek lands on the nearest keyframe instead of decoding forward to an
    // exact frame — for a thumbnail that is the right trade.
    if (typeof v.fastSeek === "function") v.fastSeek(t);
    else v.currentTime = t;
  }, []);

  useEffect(() => {
    if (!visible || !src) return;
    pendingRef.current = time;
    flush();
  }, [time, visible, src, flush]);

  const half = THUMB_W / 2;
  const left =
    trackWidth <= THUMB_W
      ? trackWidth / 2
      : Math.min(Math.max(x, half), trackWidth - half);

  return (
    <div
      className={cn(
        // pointer-events-none keeps the bar's `offsetX` relative to its root.
        "pointer-events-none absolute bottom-full z-10 mb-2.5 flex -translate-x-1/2",
        "flex-col items-center gap-1.5 transition-opacity duration-100",
        visible ? "opacity-100" : "opacity-0",
      )}
      style={{ left }}
    >
      {src && !failed && (
        <div
          className="overflow-hidden rounded-md border border-white/20 bg-black shadow-lg shadow-black/50"
          style={{ width: THUMB_W, height: Math.round(THUMB_W / aspect) }}
        >
          <video
            ref={videoRef}
            src={src}
            muted
            playsInline
            preload="metadata"
            className={cn(
              "size-full object-contain transition-opacity duration-100",
              ready ? "opacity-100" : "opacity-0",
            )}
            onLoadedMetadata={() => {
              const v = videoRef.current;
              if (v?.videoWidth && v.videoHeight) {
                setAspect(v.videoWidth / v.videoHeight);
              }
              flush();
            }}
            onSeeked={() => {
              seekingRef.current = false;
              setReady(true);
              flush();
            }}
            onError={() => setFailed(true)}
          />
        </div>
      )}
      <span className="rounded-full bg-black/75 px-1.5 py-px text-[11px] tabular-nums text-white">
        {fmtTime(Math.floor(time), forceHours)}
      </span>
    </div>
  );
}

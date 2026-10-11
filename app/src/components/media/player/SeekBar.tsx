import { useRef, useState } from "react";
import { cn } from "@/lib/utils";
import { ScrubPreview } from "@/components/media/ScrubPreview";
import { panelOnScreen } from "@/components/media/player/constants";

/** Video scrub bar. `offsetX / offsetWidth` keeps the maths in the element's
 *  own coordinates. Colours are fixed: it sits on the video scrim. */
export function SeekBar({
  value,
  max,
  previewSrc,
  chapters,
  endAt,
  onSeek,
}: {
  value: number;
  max: number;
  /** Source for the hover thumbnail; `null` until the video is downloaded. */
  previewSrc: string | null;
  /** Chapter boundaries, in seconds — notched into the track. */
  chapters: number[];
  /** Where the content ends, in seconds, when known — a flag on the track
   *  with what follows dimmed. */
  endAt: number | null;
  onSeek: (seconds: number) => void;
}) {
  const endPct = endAt != null && endAt > 0 && endAt < max ? (endAt / max) * 100 : null;
  const pct = Math.min(100, Math.max(0, (value / max) * 100));

  // `armed` mounts the preview decoder on first hover; the position is kept
  // while it fades out.
  const [armed, setArmed] = useState(false);
  const [hovering, setHovering] = useState(false);
  const [preview, setPreview] = useState({ x: 0, t: 0, w: 1 });
  /** Pointer inside the bar — a drag that ends outside it should not stick. */
  const insideRef = useRef(false);

  const trackFromEvent = (e: React.PointerEvent<HTMLDivElement>) => {
    const el = e.currentTarget;
    const x = e.nativeEvent.offsetX;
    const frac = Math.min(1, Math.max(0, x / el.offsetWidth));
    setPreview({ x, t: frac * max, w: el.offsetWidth });
    return frac * max;
  };

  return (
    <div
      role="slider"
      aria-label="Seek"
      aria-valuemin={0}
      aria-valuemax={Math.floor(max)}
      aria-valuenow={Math.floor(value)}
      className="group/seek relative flex w-full items-center h-3.5 cursor-pointer touch-none select-none"
      onPointerEnter={() => {
        insideRef.current = true;
        setArmed(true);
        setHovering(true);
      }}
      onPointerLeave={(e) => {
        insideRef.current = false;
        // Mid-drag the pointer is still ours; the preview follows it out.
        if (!e.currentTarget.hasPointerCapture(e.pointerId)) setHovering(false);
      }}
      onLostPointerCapture={() => {
        if (!insideRef.current) setHovering(false);
      }}
      onPointerDown={(e) => {
        // The click that closes a panel is not also a seek.
        if (panelOnScreen()) return;
        e.currentTarget.setPointerCapture(e.pointerId);
        setHovering(true);
        onSeek(trackFromEvent(e));
      }}
      onPointerMove={(e) => {
        const t = trackFromEvent(e);
        if (e.currentTarget.hasPointerCapture(e.pointerId)) onSeek(t);
      }}
    >
      {armed && (
        <ScrubPreview
          src={previewSrc}
          time={preview.t}
          x={preview.x}
          trackWidth={preview.w}
          forceHours={max >= 3600}
          visible={hovering}
        />
      )}

      {/* pointer-events-none children keep `offsetX` relative to the root */}
      <div
        className={cn(
          "pointer-events-none relative w-full overflow-hidden rounded-full bg-white/25",
          "h-[3px] transition-[height] group-hover/seek:h-[5px]",
        )}
      >
        {endPct != null && (
          <div
            aria-hidden
            className="absolute inset-y-0 right-0 bg-black/35"
            style={{ left: `${endPct}%` }}
          />
        )}
        <div className="absolute h-full bg-brand" style={{ width: `${pct}%` }} />
        {/* Notches, not segments, so the track keeps its rounded ends and hover
            growth. The boundary at second 0 is the left edge: not drawn. */}
        {chapters.map((t) =>
          t > 0 && t < max ? (
            <span
              key={t}
              aria-hidden
              className="absolute inset-y-0 w-[2px] -translate-x-1/2 bg-black/55"
              style={{ left: `${(t / max) * 100}%` }}
            />
          ) : null,
        )}
      </div>
      {/* Outside the track, which clips: the flag stands proud of it. */}
      {endPct != null && (
        <span
          aria-hidden
          className="pointer-events-none absolute h-[9px] w-[2px] -translate-x-1/2 rounded-full bg-white shadow-sm"
          style={{ left: `${endPct}%` }}
        />
      )}
      <div
        className={cn(
          "pointer-events-none absolute size-3 -translate-x-1/2 rounded-full bg-brand shadow-sm",
          "transition-transform group-hover/seek:scale-115",
        )}
        style={{ left: `${pct}%` }}
      />
    </div>
  );
}

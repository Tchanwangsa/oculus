import { useCallback, useEffect, useRef, useState, type RefObject } from "react";
import {
  clampOffset,
  clampPipWidth,
  clampSplit,
  PIP_MAX_W,
  PIP_MIN_PX_H,
  PIP_MIN_PX_W,
  PIP_MIN_W,
  usePlayerPrefs,
} from "@/stores/playerPrefsStore";
import { capturePress, useFrameDrag } from "@/hooks/useFrameDrag";

/** The corner a resize is pulling from; the opposite one stays put. */
export type PipCorner = "nw" | "ne" | "sw" | "se";

export const PIP_CORNERS: PipCorner[] = ["nw", "ne", "sw", "se"];

/** Fallback until a picture reports its own dimensions. */
const DEFAULT_ASPECT = 16 / 9;

type Drag =
  | { kind: "move"; px: number; py: number; x: number; y: number }
  | { kind: "resize"; corner: PipCorner; px: number; w: number; x: number; y: number }
  | { kind: "split"; py: number; v: number };

/**
 * Geometry for the two-source layouts (PIP box, stack split), stored as
 * fractions of the video area so it survives peek ↔ fullscreen. The PIP's
 * height is never stored: CSS `aspect-ratio` locks it.
 */
export function useSourceLayout(
  areaRef: RefObject<HTMLElement | null>,
  /** Width ÷ height of the picture in the PIP box. */
  aspect = DEFAULT_ASPECT,
) {
  const pipX = usePlayerPrefs((s) => s.pipX);
  const pipY = usePlayerPrefs((s) => s.pipY);
  const pipW = usePlayerPrefs((s) => s.pipW);
  const split = usePlayerPrefs((s) => s.split);
  const setPrefs = usePlayerPrefs((s) => s.set);

  const [dragKind, setDragKind] = useState<Drag["kind"] | null>(null);
  const drag = useRef<Drag | null>(null);
  // Area in pixels: a ref for mid-drag maths, state so the floor re-clamps.
  const size = useRef({ w: 0, h: 0 });
  const [area, setArea] = useState({ w: 0, h: 0 });

  useEffect(() => {
    const el = areaRef.current;
    if (!el) return;
    const measure = () => {
      const next = { w: el.clientWidth, h: el.clientHeight };
      size.current = next;
      setArea((prev) => (prev.w === next.w && prev.h === next.h ? prev : next));
    };
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, [areaRef]);

  // Fraction floor plus a pixel floor (the height one via the aspect); the
  // `PIP_MAX_W` cap still wins on a tiny area.
  const clampWidth = useCallback(
    (w: number) => {
      const clamped = clampPipWidth(w);
      if (!area.w || !area.h) return clamped;
      const floor = Math.min(
        PIP_MAX_W,
        Math.max(PIP_MIN_W, PIP_MIN_PX_W / area.w, (PIP_MIN_PX_H * aspect) / area.w),
      );
      return Math.max(floor, clamped);
    },
    [area.w, area.h, aspect],
  );

  /** The PIP's height as a fraction of the area, from its locked aspect. */
  const pipHeight = useCallback(
    (w: number) => {
      const { w: aw, h: ah } = size.current;
      // Before the first measure, assume a 16:9 area.
      if (!aw || !ah) return (w / aspect) * DEFAULT_ASPECT;
      return (w * aw) / aspect / ah;
    },
    [aspect],
  );

  const begin = (e: React.PointerEvent, next: Drag) => {
    if (!capturePress(e)) return false;
    e.stopPropagation();
    drag.current = next;
    setDragKind(next.kind);
    document.body.style.userSelect = "none";
    return true;
  };

  const startPipMove = useCallback(
    (e: React.PointerEvent) => {
      begin(e, { kind: "move", px: e.clientX, py: e.clientY, x: pipX, y: pipY });
    },
    [pipX, pipY],
  );

  const startPipResize = useCallback(
    (e: React.PointerEvent, corner: PipCorner) => {
      begin(e, { kind: "resize", corner, px: e.clientX, w: pipW, x: pipX, y: pipY });
    },
    [pipW, pipX, pipY],
  );

  const startSplitDrag = useCallback(
    (e: React.PointerEvent) => {
      if (begin(e, { kind: "split", py: e.clientY, v: split })) {
        document.body.style.cursor = "row-resize";
      }
    },
    [split],
  );

  useFrameDrag({
    active: () => !!drag.current,
    move: (e) => {
      const d = drag.current;
      if (!d) return;
      const { w: aw, h: ah } = size.current;
      if (!aw || !ah) return;

      if (d.kind === "move") {
        const w = clampWidth(pipW);
        const x = d.x + (e.clientX - d.px) / aw;
        const y = d.y + (e.clientY - d.py) / ah;
        setPrefs({
          pipX: clampOffset(x, w),
          pipY: clampOffset(y, pipHeight(w)),
        });
        return;
      }

      if (d.kind === "resize") {
        const towardsRight = d.corner === "ne" || d.corner === "se";
        const dx = ((e.clientX - d.px) / aw) * (towardsRight ? 1 : -1);
        const w = clampWidth(d.w + dx);
        const grew = w - d.w;
        const h = pipHeight(w);
        // Anchor the corner opposite the one being dragged.
        const x = towardsRight ? d.x : d.x - grew;
        const y =
          d.corner === "se" || d.corner === "sw" ? d.y : d.y - (h - pipHeight(d.w));
        setPrefs({ pipW: w, pipX: clampOffset(x, w), pipY: clampOffset(y, h) });
        return;
      }

      setPrefs({ split: clampSplit(d.v + (e.clientY - d.py) / ah) });
    },
    end: () => {
      drag.current = null;
      setDragKind(null);
      document.body.style.cursor = "";
      document.body.style.userSelect = "";
    },
  });

  // Clamped here too, so a width stored fullscreen stays legible in a peek.
  const boxW = clampWidth(pipW);
  const pipStyle: React.CSSProperties = {
    left: `${clampOffset(pipX, boxW) * 100}%`,
    top: `${clampOffset(pipY, pipHeight(boxW)) * 100}%`,
    width: `${boxW * 100}%`,
    aspectRatio: String(aspect),
  };

  return {
    pipStyle,
    split,
    pipDragging: dragKind === "move" || dragKind === "resize",
    splitting: dragKind === "split",
    startPipMove,
    startPipResize,
    startSplitDrag,
  };
}

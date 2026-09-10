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
 * Geometry for the two-source layouts: where the picture-in-picture box sits
 * and how the stacked view divides its height. Both are stored as **fractions
 * of the video area** in `playerPrefsStore`, not pixels — the player is a peek
 * panel one moment and a fullscreen overlay the next, and an inset pinned at
 * "320px from the left" means something different in each.
 *
 * The PIP's height is never stored. It comes from `aspect-ratio` on the box, so
 * the ratio is locked by construction rather than by arithmetic that has to be
 * kept right in three places; this hook only needs the aspect to know how much
 * vertical room the box takes when clamping it inside the frame.
 *
 * Pointer maths stays in client coordinates against one `getBoundingClientRect`
 * of the video area — safe here because the app zooms the webview page rather
 * than CSS-zooming a container (see docs/frontend.md).
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

  /** What is being dragged, in state because the frames restyle mid-drag. The
   *  two are kept apart so a PIP drag does not light up the stack divider. */
  const [dragKind, setDragKind] = useState<Drag["kind"] | null>(null);
  const drag = useRef<Drag | null>(null);
  /** Area size in pixels, for turning pointer travel into fractions. Kept in
   *  a ref for the pointer maths, which needs the freshest value mid-drag,
   *  and in state for the pixel floor below, which has to re-clamp when the
   *  player is resized. */
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

  /**
   * The stored width is a fraction, and a fraction is not a size: the same
   * 26% is a legible inset over a fullscreen lecture and an illegible one in
   * a peek panel. So the fraction floor gets a pixel floor on top, in both
   * directions — the height one arrives as a width through the locked aspect,
   * since width is the only number the box is actually given.
   *
   * The cap still wins: on an area narrower than the floor itself, the PIP is
   * `PIP_MAX_W` of it rather than wider than the picture behind it.
   */
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
      // Before the first measure, assume the area is itself roughly 16:9 —
      // only used to clamp `y`, and one frame later the real numbers arrive.
      if (!aw || !ah) return (w / aspect) * DEFAULT_ASPECT;
      return (w * aw) / aspect / ah;
    },
    [aspect],
  );

  const begin = (e: React.PointerEvent, next: Drag) => {
    if (e.button !== 0) return false;
    e.preventDefault();
    e.stopPropagation();
    (e.currentTarget as HTMLElement).setPointerCapture?.(e.pointerId);
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

  useEffect(() => {
    // A trackpad emits pointermove faster than the display refreshes, and each
    // one here sets state that re-lays-out the player. Coalescing to one frame
    // is the difference between a smooth drag and several layouts per frame.
    let frame = 0;
    let pending: { clientX: number; clientY: number } | null = null;

    const apply = (e: { clientX: number; clientY: number }) => {
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
        // Width alone drives the box; the aspect gives the height, so a corner
        // drag can only ever produce a similar rectangle.
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
    };

    const onMove = (e: PointerEvent) => {
      if (!drag.current) return;
      pending = { clientX: e.clientX, clientY: e.clientY };
      if (frame) return;
      frame = requestAnimationFrame(() => {
        frame = 0;
        if (pending) apply(pending);
        pending = null;
      });
    };

    const onUp = () => {
      if (!drag.current) return;
      drag.current = null;
      setDragKind(null);
      document.body.style.cursor = "";
      document.body.style.userSelect = "";
    };

    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    window.addEventListener("pointercancel", onUp);
    return () => {
      if (frame) cancelAnimationFrame(frame);
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      window.removeEventListener("pointercancel", onUp);
    };
  }, [pipW, pipHeight, clampWidth, setPrefs]);

  /** Ready to spread onto the PIP box. `aspectRatio` is what locks the shape.
   *  The width is clamped here as well as on the drag, so a box stored when
   *  the player was fullscreen still comes back legible in a peek panel. */
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

import { useCallback, useEffect, useRef, useState, type RefObject } from "react";
import { cn } from "@/lib/utils";

const POS_KEY = "oculus-lecture-caption-pos";
/** Default: centred, just above the bottom edge of the video. */
const DEFAULT_POS = { x: 0.5, y: 0.94 };

type Pos = { x: number; y: number };

const clamp = (n: number) => Math.min(1, Math.max(0, n));

function loadPos(): Pos {
  try {
    const raw = localStorage.getItem(POS_KEY);
    if (raw) {
      const [x, y] = raw.split(",").map(Number);
      if (Number.isFinite(x) && Number.isFinite(y)) {
        return { x: clamp(x), y: clamp(y) };
      }
    }
  } catch {
    /* ignore */
  }
  return DEFAULT_POS;
}

/**
 * The caption line over the video, draggable off whatever the slide has at the
 * bottom. Position is a fraction of the *free* space in the video box
 * (`left: x%` + `translateX(-x%)`), so it survives resizes and longer lines.
 */
export function CaptionOverlay({
  text,
  boundsRef,
  lift = 0,
}: {
  text: string;
  boundsRef: RefObject<HTMLElement | null>;
  /** Pixels to ride up by while the control bar is showing under it. */
  lift?: number;
}) {
  const [pos, setPos] = useState<Pos>(loadPos);
  // Mirrored for the pointer handlers, which must not re-subscribe mid-drag.
  const liftRef = useRef(lift);
  liftRef.current = lift;
  const ref = useRef<HTMLDivElement>(null);
  const grab = useRef<{ gx: number; gy: number; w: number; h: number } | null>(null);
  const [dragging, setDragging] = useState(false);

  useEffect(() => {
    try {
      localStorage.setItem(POS_KEY, `${pos.x.toFixed(4)},${pos.y.toFixed(4)}`);
    } catch {
      /* ignore */
    }
  }, [pos]);

  const onPointerDown = useCallback((e: React.PointerEvent) => {
    if (e.button !== 0) return;
    const rect = ref.current?.getBoundingClientRect();
    if (!rect) return;
    e.preventDefault();
    e.currentTarget.setPointerCapture?.(e.pointerId);
    grab.current = {
      gx: e.clientX - rect.left,
      gy: e.clientY - rect.top,
      w: rect.width,
      h: rect.height,
    };
    setDragging(true);
  }, []);

  useEffect(() => {
    if (!dragging) return;
    const onMove = (e: PointerEvent) => {
      const g = grab.current;
      const bounds = boundsRef.current?.getBoundingClientRect();
      if (!g || !bounds) return;
      const freeX = bounds.width - g.w;
      const freeY = bounds.height - g.h;
      // `pos` is unlifted but the grab offset came from the lifted rect: add the
      // lift back.
      const top = e.clientY - g.gy - bounds.top + liftRef.current;
      setPos({
        x: freeX > 0 ? clamp((e.clientX - g.gx - bounds.left) / freeX) : 0.5,
        y: freeY > 0 ? clamp(top / freeY) : 0.5,
      });
    };
    const onUp = () => {
      grab.current = null;
      setDragging(false);
      document.body.style.userSelect = "";
    };
    document.body.style.userSelect = "none";
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    window.addEventListener("pointercancel", onUp);
    return () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      window.removeEventListener("pointercancel", onUp);
    };
  }, [dragging, boundsRef]);

  return (
    <div
      ref={ref}
      onPointerDown={onPointerDown}
      onDoubleClick={() => setPos(DEFAULT_POS)}
      title="Drag to move · double-click to reset"
      className={cn(
        "absolute z-20 w-max max-w-[85%] select-none cursor-grab active:cursor-grabbing",
        // Only the lift animates; a drag must track the pointer exactly.
        !dragging && "transition-transform duration-200",
      )}
      style={{
        left: `${pos.x * 100}%`,
        top: `${pos.y * 100}%`,
        transform: `translate(${-pos.x * 100}%, ${-pos.y * 100}%) translateY(${-lift}px)`,
      }}
    >
      <span
        className={cn(
          "block px-3 py-1.5 rounded text-sm text-white text-center leading-snug",
          dragging ? "bg-brand/85" : "bg-black/75",
        )}
        style={{ textShadow: "0 1px 2px rgba(0,0,0,0.8)" }}
      >
        {text}
      </span>
    </div>
  );
}

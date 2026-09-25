import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import {
  ArrowsIn,
  MagnifyingGlassMinus,
  MagnifyingGlassPlus,
  X,
} from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogCanvas,
  DialogDescription,
  DialogTitle,
} from "@/components/ui/dialog";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { cn } from "@/lib/utils";

/**
 * A full-window, zoomable, pannable viewer for something drawn small — a
 * mermaid figure (`DiagramLightbox`) or an attached picture (`ImageLightbox`).
 *
 * Panning is the container's own scroll (free momentum, scrollbars, keys);
 * only the scale is a `transform`, on a host of fixed natural size inside a
 * layout box sized `natural × zoom`. A transform, not CSS `zoom` (see
 * CLAUDE.md): under a transform pointer coords and rects share one space, so
 * the anchor maths below holds, and the content never re-lays out.
 */

const MIN_ZOOM = 0.1;
const MAX_ZOOM = 8;
const STEP = 1.25;

/** Cap on the initial fit, so a small diagram fills the window without
 *  looking blown up. */
const MAX_FIT = 2.5;

/** Gutter at fit, in px: `p-9` each side; the bottom also clears the
 *  floating toolbar (`pb-20`). */
const GUTTER_X = 72;
const GUTTER_Y = 36 + 80;

/**
 * Time constant of the zoom smoother, in ms. Inputs arrive in steps (a wheel
 * notch, a quantised pinch `scale`, a button's 1.25×), so they only move a
 * target and the painted zoom eases towards it every frame.
 */
const SMOOTH_MS = 45;

/** Zoom per pixel of ⌘-scroll, as an exponent (zoom is multiplicative). A
 *  macOS wheel notch is ±120px, so this makes one notch ~1.3×. */
const WHEEL_GAIN = 0.0022;

/** Per-event cap, so trackpad momentum can't cross the range in a frame. */
const WHEEL_MAX_STEP = 1.3;

/** `gestureend` can go undelivered (pinch ends off-window), so the pinch is
 *  assumed over this long after its last event. */
const GESTURE_IDLE_MS = 400;

const clampZoom = (z: number) => Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, z));

/** The content's natural, unscaled size — all viewer maths is in it. */
export type LightboxSize = { width: number; height: number };

export function Lightbox({
  size,
  open,
  onOpenChange,
  title,
  children,
  scrollerClassName,
  selectableSelector,
}: {
  size: LightboxSize;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** Screen-reader only. */
  title: string;
  /** Drawn at `size`, scaled by a transform. */
  children: React.ReactNode;
  /** E.g. the diagram restores its labels' I-beam and selection here. */
  scrollerClassName?: string;
  /** A press on a match selects text instead of starting a pan. */
  selectableSelector?: string;
}) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogCanvas
        // Focus the canvas, not the toolbar's first button, so keys work at once.
        onOpenAutoFocus={(e) => {
          e.preventDefault();
          (e.currentTarget as HTMLElement).querySelector<HTMLElement>("[data-canvas]")?.focus();
        }}
      >
        <DialogTitle className="sr-only">{title}</DialogTitle>
        <DialogDescription className="sr-only">
          Scroll or drag to pan, ⌘-scroll or pinch to zoom, Escape to close.
        </DialogDescription>
        <Viewer
          size={size}
          onClose={() => onOpenChange(false)}
          scrollerClassName={scrollerClassName}
          selectableSelector={selectableSelector}
        >
          {children}
        </Viewer>
      </DialogCanvas>
    </Dialog>
  );
}

/** Which clamp the zoom is at, for greying a button — the only React state a
 *  zoom touches. */
type Limit = "none" | "min" | "max";

/** Mounted per open, so every view starts at fit. */
function Viewer({
  size,
  onClose,
  children,
  scrollerClassName,
  selectableSelector,
}: {
  size: LightboxSize;
  onClose: () => void;
  children: React.ReactNode;
  scrollerClassName?: string;
  selectableSelector?: string;
}) {
  const scroller = useRef<HTMLDivElement>(null);
  /** The layout box, sized `natural × zoom` so the scroll extent is right. */
  const box = useRef<HTMLDivElement>(null);
  const host = useRef<HTMLDivElement>(null);
  /** The toolbar percentage, written directly by `paint`. */
  const readout = useRef<HTMLSpanElement>(null);

  // Zoom lives in refs (it moves faster than React renders): `target` is what
  // input asked for, `current` what the smoother has eased to, `painted` what
  // the DOM shows — the scale every rect below is measured in.
  const target = useRef(1);
  const current = useRef(1);
  const painted = useRef<number | null>(null);
  const fit = useRef(1);

  const [limit, setLimit] = useState<Limit>("none");

  // The point (content coords) to hold still at screen position (px, py);
  // scale-free, so it is re-applied every frame of the ease.
  const anchor = useRef<{ x: number; y: number; px: number; py: number } | null>(null);

  const raf = useRef<number | null>(null);
  const stamp = useRef<number | null>(null);

  const fitZoom = useCallback(() => {
    const el = scroller.current;
    if (!el) return 1;
    const wide = (el.clientWidth - GUTTER_X) / size.width;
    const tall = (el.clientHeight - GUTTER_Y) / size.height;
    return clampZoom(Math.min(wide, tall, MAX_FIT));
  }, [size.width, size.height]);

  /** Put a scale on screen and apply the anchor. The only DOM writer for a
   *  zoom; React never owns these properties. */
  const paint = useCallback(
    (z: number) => {
      const el = scroller.current;
      const layout = box.current;
      const picture = host.current;
      if (!el || !layout || !picture) return;
      layout.style.width = `${size.width * z}px`;
      layout.style.height = `${size.height * z}px`;
      picture.style.transform = `scale(${z})`;
      painted.current = z;

      const a = anchor.current;
      if (a) {
        // Measure where the anchor landed and correct by the difference —
        // robust to `m-auto` collapsing and the gutter, which a computed
        // offset would get wrong as the picture crosses the window's size.
        const rect = picture.getBoundingClientRect();
        el.scrollLeft += rect.left + a.x * z - a.px;
        el.scrollTop += rect.top + a.y * z - a.py;
      }

      if (readout.current) readout.current.textContent = `${Math.round(z * 100)}%`;
      const at: Limit = z >= MAX_ZOOM ? "max" : z <= MIN_ZOOM ? "min" : "none";
      setLimit((was) => (was === at ? was : at));
    },
    [size.width, size.height],
  );

  const stop = useCallback(() => {
    if (raf.current != null) cancelAnimationFrame(raf.current);
    raf.current = null;
    stamp.current = null;
  }, []);

  /** Ease towards `target` per frame; time-based so 60Hz and 120Hz match. */
  const run = useCallback(() => {
    if (raf.current != null) return;
    const tick = (ts: number) => {
      raf.current = null;
      const dt = stamp.current == null ? 16 : Math.min(64, ts - stamp.current);
      stamp.current = ts;
      const to = target.current;
      let z = current.current + (to - current.current) * (1 - Math.exp(-dt / SMOOTH_MS));
      // Snap when close rather than approaching forever.
      if (Math.abs(to - z) < to * 0.0015) z = to;
      current.current = z;
      paint(z);
      if (z !== to) raf.current = requestAnimationFrame(tick);
      else {
        stamp.current = null;
        anchor.current = null;
      }
    };
    raf.current = requestAnimationFrame(tick);
  }, [paint]);

  useEffect(() => stop, [stop]);

  /** Zoom holding `(clientX, clientY)` still (window centre if omitted).
   *  Callers compose on `target`, not the painted zoom, so bursts add up. */
  const requestZoom = useCallback(
    (next: number, clientX?: number, clientY?: number) => {
      const el = scroller.current;
      const picture = host.current;
      // `painted`, not `current`: the rects below are in the on-screen scale.
      const now = painted.current;
      if (!el || !picture || now == null) return;
      const wanted = clampZoom(next);
      if (wanted === target.current) return;
      const view = el.getBoundingClientRect();
      const px = clientX ?? view.left + view.width / 2;
      const py = clientY ?? view.top + view.height / 2;
      const rect = picture.getBoundingClientRect();
      anchor.current = {
        x: (px - rect.left) / now,
        y: (py - rect.top) / now,
        px,
        py,
      };
      target.current = wanted;
      run();
    },
    [run],
  );

  const reset = useCallback(() => {
    const next = fitZoom();
    fit.current = next;
    anchor.current = null;
    target.current = next;
    current.current = next;
    stop();
    paint(next);
    const el = scroller.current;
    if (el) {
      el.scrollLeft = 0;
      el.scrollTop = 0;
    }
  }, [fitZoom, paint, stop]);

  // Fit before first paint, and only on open — `reset` is also the Fit button.
  useLayoutEffect(() => {
    reset();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Zoom gestures. WKWebView reports a pinch both as `gesture*` events and as
  // synthesized `ctrlKey` wheels; while `gestureActive`, the wheel path is
  // ignored so the pinch isn't applied twice. Plain scrolling is the pan.
  useEffect(() => {
    const el = scroller.current;
    if (!el) return;
    let pinchBase = 1;
    let gestureActive = false;
    let idle: number | null = null;

    const keepAlive = () => {
      if (idle != null) clearTimeout(idle);
      idle = window.setTimeout(() => {
        gestureActive = false;
        idle = null;
      }, GESTURE_IDLE_MS);
    };

    const onWheel = (e: WheelEvent) => {
      if (!e.ctrlKey && !e.metaKey) return;
      e.preventDefault();
      if (gestureActive) return;
      // `deltaMode === 1` is lines, not pixels.
      const dy = e.deltaY * (e.deltaMode === 1 ? 16 : 1);
      const stepBy = Math.min(
        WHEEL_MAX_STEP,
        Math.max(1 / WHEEL_MAX_STEP, Math.exp(-dy * WHEEL_GAIN)),
      );
      requestZoom(target.current * stepBy, e.clientX, e.clientY);
    };
    const onGestureStart = (e: Event) => {
      e.preventDefault();
      gestureActive = true;
      pinchBase = target.current;
      keepAlive();
    };
    const onGestureChange = (e: Event) => {
      e.preventDefault();
      const g = e as unknown as { scale: number; clientX: number; clientY: number };
      keepAlive();
      // `scale` is cumulative for the gesture, not a delta.
      if (g.scale) requestZoom(pinchBase * g.scale, g.clientX, g.clientY);
    };
    const onGestureEnd = (e: Event) => {
      e.preventDefault();
      gestureActive = false;
      if (idle != null) clearTimeout(idle);
      idle = null;
    };

    el.addEventListener("wheel", onWheel, { passive: false });
    el.addEventListener("gesturestart", onGestureStart);
    el.addEventListener("gesturechange", onGestureChange);
    el.addEventListener("gestureend", onGestureEnd);
    return () => {
      if (idle != null) clearTimeout(idle);
      el.removeEventListener("wheel", onWheel);
      el.removeEventListener("gesturestart", onGestureStart);
      el.removeEventListener("gesturechange", onGestureChange);
      el.removeEventListener("gestureend", onGestureEnd);
    };
  }, [requestZoom]);

  // A window resize re-measures fit but leaves the view where it is.
  useEffect(() => {
    const el = scroller.current;
    if (!el || typeof ResizeObserver === "undefined") return;
    const ro = new ResizeObserver(() => {
      fit.current = fitZoom();
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, [fitZoom]);

  // Drag to pan, on pointer events (see CLAUDE.md: HTML5 drag).
  const drag = useRef<{ x: number; y: number; left: number; top: number } | null>(null);
  const [dragging, setDragging] = useState(false);

  const onPointerDown = (e: React.PointerEvent) => {
    if (e.button !== 0) return;
    const el = scroller.current;
    if (!el) return;
    if (selectableSelector && (e.target as Element).closest?.(selectableSelector)) return;
    // Stops a selection-drag and an `<img>`'s native drag; it also cancels
    // the focus move, so focus the canvas by hand.
    e.preventDefault();
    el.focus();
    // An in-flight ease must stop re-anchoring against the pan.
    anchor.current = null;
    drag.current = { x: e.clientX, y: e.clientY, left: el.scrollLeft, top: el.scrollTop };
    setDragging(true);
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  };
  const onPointerMove = (e: React.PointerEvent) => {
    const el = scroller.current;
    const d = drag.current;
    if (!el || !d) return;
    el.scrollLeft = d.left - (e.clientX - d.x);
    el.scrollTop = d.top - (e.clientY - d.y);
  };
  const endDrag = (e: React.PointerEvent) => {
    drag.current = null;
    setDragging(false);
    const el = e.currentTarget as HTMLElement;
    if (el.hasPointerCapture(e.pointerId)) el.releasePointerCapture(e.pointerId);
  };

  // Escape is Radix's; arrows fall through to the scroller.
  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "+" || e.key === "=") {
      e.preventDefault();
      requestZoom(target.current * STEP);
    } else if (e.key === "-" || e.key === "_") {
      e.preventDefault();
      requestZoom(target.current / STEP);
    } else if (e.key === "0") {
      e.preventDefault();
      reset();
    }
  };

  // Double-click toggles fit ↔ 100% at the pointer.
  const onDoubleClick = (e: React.MouseEvent) => {
    if (Math.abs(target.current - fit.current) < 0.01) requestZoom(1, e.clientX, e.clientY);
    else reset();
  };

  return (
    <>
      <div
        ref={scroller}
        data-canvas
        tabIndex={-1}
        onKeyDown={onKeyDown}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={endDrag}
        onPointerCancel={endDrag}
        onDoubleClick={onDoubleClick}
        className={cn(
          // `overflow-scroll`, not `auto`: scrollbars appearing mid-zoom would
          // change `clientWidth` and re-centre under the pinch
          // (`scrollbar-gutter` is a no-op in WebKit — see CLAUDE.md).
          "flex flex-1 overflow-scroll outline-none",
          scrollerClassName,
          dragging
            ? // `!` to beat the more specific rules `scrollerClassName` gives
              // the content (the diagram's `svg text`).
              "cursor-grabbing [&_*]:cursor-grabbing! [&_*]:select-none!"
            : "cursor-grab",
        )}
      >
        <div
          ref={box}
          // No `style` prop: `paint` owns the size. `m-auto`, not
          // `justify-center`, so an oversized picture's start edge stays
          // scrollable. `shrink-0` or flex squeezes it back to the container.
          // `box-content` keeps the gutter outside the zoomed size.
          className="m-auto box-content shrink-0 p-9 pb-20"
        >
          <div
            ref={host}
            // Natural size, scaled from the corner to sit flush in `box`.
            style={{
              width: size.width,
              height: size.height,
              transformOrigin: "0 0",
              willChange: "transform",
            }}
          >
            {children}
          </div>
        </div>
      </div>

      <Toolbar
        readout={readout}
        limit={limit}
        onIn={() => requestZoom(target.current * STEP)}
        onOut={() => requestZoom(target.current / STEP)}
        onReset={reset}
        onClose={onClose}
      />
    </>
  );
}

/** Floating controls, same grammar as `PDFViewer`'s zoom cluster. The
 *  percentage is a ref so a zoom frame renders no React. */
function Toolbar({
  readout,
  limit,
  onIn,
  onOut,
  onReset,
  onClose,
}: {
  readout: React.RefObject<HTMLSpanElement | null>;
  limit: Limit;
  onIn: () => void;
  onOut: () => void;
  onReset: () => void;
  onClose: () => void;
}) {
  return (
    <div className="pointer-events-none absolute inset-x-0 bottom-6 flex justify-center">
      <div className="pointer-events-auto flex items-center gap-0.5 rounded-full border border-border bg-card/90 p-1 text-xs text-muted-foreground shadow-md backdrop-blur-sm">
        <Action label="Zoom out" onClick={onOut} disabled={limit === "min"}>
          <MagnifyingGlassMinus size={14} />
        </Action>
        <button
          type="button"
          onClick={onReset}
          className="w-12 cursor-pointer text-center tabular-nums transition-colors hover:text-foreground"
          aria-label="Fit to window"
        >
          {/* Empty on purpose: `paint` owns the text; a JSX child would be
              restored stale on the next `limit` re-render. */}
          <span ref={readout} />
        </button>
        <Action label="Zoom in" onClick={onIn} disabled={limit === "max"}>
          <MagnifyingGlassPlus size={14} />
        </Action>
        <Action label="Fit to window" onClick={onReset}>
          <ArrowsIn size={14} />
        </Action>
        <div className="mx-1 h-4 w-px bg-border" />
        <Action label="Close" onClick={onClose}>
          <X size={14} />
        </Action>
      </div>
    </div>
  );
}

function Action({
  label,
  onClick,
  disabled,
  children,
}: {
  label: string;
  onClick: () => void;
  disabled?: boolean;
  children: React.ReactNode;
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button variant="ghost" size="icon-sm" onClick={onClick} disabled={disabled} aria-label={label}>
          {children}
        </Button>
      </TooltipTrigger>
      <TooltipContent>{label}</TooltipContent>
    </Tooltip>
  );
}

/** A picture, opened out. Measures its natural size first (from cache) and
 *  mounts the viewer only then — a fit against 0×0 would open at a clamp. */
export function ImageLightbox({
  src,
  alt,
  open,
  onOpenChange,
}: {
  src: string;
  alt?: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const [size, setSize] = useState<LightboxSize | null>(null);

  useEffect(() => {
    if (!open || !src) {
      setSize(null);
      return;
    }
    let live = true;
    const probe = new Image();
    probe.onload = () => {
      if (live) setSize({ width: probe.naturalWidth, height: probe.naturalHeight });
    };
    probe.src = src;
    return () => {
      live = false;
    };
  }, [open, src]);

  if (!open || !size) return null;

  return (
    <Lightbox size={size} open={open} onOpenChange={onOpenChange} title={alt || "Picture"}>
      <img src={src} alt={alt ?? ""} draggable={false} className="h-full w-full" />
    </Lightbox>
  );
}

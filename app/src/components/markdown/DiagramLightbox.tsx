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
 * A diagram, opened out: the whole window, zoomable and pannable.
 *
 * The inline figure in a reply is a picture sized to the column it is in
 * (`Mermaid.tsx`), which is right for reading past and wrong for reading
 * *into* — a twenty-node flowchart in a 360px dock is a shape, not a diagram.
 * This is where it becomes one.
 *
 * **Panning is the container's own scroll; only the scale is a transform.**
 * The picture sits in an `overflow-scroll` box, which is what `PDFViewer` does
 * and for the same payoff: two-finger panning, momentum, scrollbars and
 * keyboard scrolling all arrive for free and behave the way every other
 * scroller in the app does. What the zoom changes is a `scale()` on an
 * SVG host of fixed size, inside a layout box that carries `natural × zoom`
 * so the scroll extent still tells the truth.
 *
 * That split is not the CSS-`zoom` mistake the app made once before
 * (`AppLayout`, root `CLAUDE.md`). Inside a CSS-`zoom`ed subtree WebKit
 * reports pointer coordinates in visual pixels and element rects in layout
 * pixels, so the two disagree; under a `transform` both are visual, so the
 * anchor maths below — pointer position against `getBoundingClientRect` — is
 * measuring one space. Measured, scaling the transform rather than resizing
 * the `<svg>` is about 3× cheaper per frame (0.3ms against 1.0ms for this
 * app's flowcharts), because the SVG's own layout never re-runs.
 */

const MIN_ZOOM = 0.1;
const MAX_ZOOM = 8;
const STEP = 1.25;

/** How far out the picture is allowed to start. A small diagram is blown up
 *  to fill the window — that is what opening it was for — but only so far
 *  before the strokes stop looking drawn and start looking zoomed. */
const MAX_FIT = 2.5;

/** Breathing room around the picture at fit, in px, and it is not square.
 *  Sideways it is `p-9` twice over. Downwards the toolbar floats *over* the
 *  picture, so a symmetric gutter fitted the diagram's last node neatly behind
 *  it — the bottom has to clear `bottom-6` plus the control's own height. */
const GUTTER_X = 72;
const GUTTER_Y = 36 + 80;

/**
 * Time constant of the zoom smoother, in ms.
 *
 * **This is what makes the zoom smooth, and it is deliberately render-side.**
 * Measured, the work of a zoom frame is about 0.3ms and not one frame is
 * dropped at 120Hz — so a juddering zoom was never the drawing being slow, it
 * was the *number* moving in steps: a mouse notch is one ±120 lurch, a
 * `gesturechange` delivers a quantised `scale`, and a button is a single 1.25×
 * jump. Chasing each input source into behaving was tried once and did not
 * hold. So the inputs only ever move a **target**, and what is painted eases
 * towards it every frame, which is smooth whatever arrived and however often.
 *
 * 45ms settles inside ~130ms: fast enough that a pinch still feels attached to
 * the fingers, slow enough that one wheel notch reads as a movement rather
 * than a cut.
 */
const SMOOTH_MS = 45;

/**
 * Zoom per pixel of ⌘-scroll, as an exponent.
 *
 * Multiplicative, because zoom is — `1 - deltaY / 100` hits zero at a deltaY
 * of 100 and goes negative past it. The constant is small on purpose: one
 * notch of a real mouse wheel on macOS is ±120, and an earlier 0.01 here
 * turned that single notch into a 1.65× jump, which is most of the "glitchy"
 * in a mouse zoom. At 0.0022 a notch is ~1.3×, and a trackpad's fractional
 * deltas land where they should — a whole ⌘-scroll gesture sums to about 2×.
 */
const WHEEL_GAIN = 0.0022;

/** Ceiling on what one wheel event may do, so a violent flick of trackpad
 *  momentum cannot cross the zoom range in a frame. */
const WHEEL_MAX_STEP = 1.3;

/** A pinch that ends off-window, or with the dialog closing under it, can
 *  leave `gestureend` undelivered — and a latched flag would mute the wheel
 *  path for the rest of the session. This is how long after the last gesture
 *  event the pinch is assumed over. */
const GESTURE_IDLE_MS = 400;

const clampZoom = (z: number) => Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, z));

export type DiagramSize = { width: number; height: number };

export function DiagramLightbox({
  svg,
  size,
  open,
  onOpenChange,
}: {
  /** The rendered SVG, already re-scoped by the caller so its ids cannot
   *  collide with the copy still on screen in the reply. */
  svg: string;
  size: DiagramSize;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogCanvas
        // Radix moves focus to the first focusable thing on open, which is the
        // toolbar's first button — that would put a focus ring on a control
        // nobody pressed. The canvas takes it instead, which is also what
        // makes the arrow keys work without a click first.
        onOpenAutoFocus={(e) => {
          e.preventDefault();
          (e.currentTarget as HTMLElement).querySelector<HTMLElement>("[data-canvas]")?.focus();
        }}
      >
        <DialogTitle className="sr-only">Diagram</DialogTitle>
        <DialogDescription className="sr-only">
          Scroll or drag to pan, ⌘-scroll or pinch to zoom, Escape to close.
        </DialogDescription>
        <Viewer svg={svg} size={size} onClose={() => onOpenChange(false)} />
      </DialogCanvas>
    </Dialog>
  );
}

/** Which end of the range the zoom is sitting on, so the toolbar can grey the
 *  button that would do nothing. A three-way state rather than the zoom
 *  itself: it changes a handful of times in a session, where the zoom changes
 *  every frame, and this is the only thing a zoom renders React for. */
type Limit = "none" | "min" | "max";

/** Split out so every ref and every piece of view state is born with the
 *  dialog and dies with it — an opened diagram always starts at fit, never at
 *  wherever the last one was left. */
function Viewer({
  svg,
  size,
  onClose,
}: {
  svg: string;
  size: DiagramSize;
  onClose: () => void;
}) {
  const scroller = useRef<HTMLDivElement>(null);
  /** The layout box. Carries `natural × zoom` so the scroller has something
   *  the right size to scroll, and the gutter outside that via `box-content`. */
  const box = useRef<HTMLDivElement>(null);
  /** The SVG's host, always at natural size and scaled by a transform. */
  const host = useRef<HTMLDivElement>(null);
  /** The percentage in the toolbar, written straight to the text node — see
   *  `SMOOTH_MS` on why a zoom renders nothing. It starts empty and is filled
   *  by the first `paint`, which runs in a layout effect, so it is never seen
   *  blank. */
  const readout = useRef<HTMLSpanElement>(null);

  // ── The zoom, which lives in refs ─────────────────────────────────────────
  // Three numbers, and the distinction between them is the whole design.
  // `target` is where the input asked to go, `current` is what the smoother
  // has eased to, and `painted` is what the DOM has actually been told — the
  // scale `scrollLeft` and every rect below are expressed in. Only refs,
  // because they move faster than React renders and the last write must win.
  const target = useRef(1);
  const current = useRef(1);
  const painted = useRef<number | null>(null);
  const fit = useRef(1);

  const [limit, setLimit] = useState<Limit>("none");

  // Where to hold the picture still while the zoom moves: a point in the
  // diagram's own coordinates, and where on screen it should stay. Scale-free
  // by construction, so the smoother can re-apply the same anchor on every
  // frame of an eased zoom rather than predicting one offset up front.
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

  /** Put a scale on screen, and spend the anchor against it.
   *
   *  Every write to the DOM in this component goes through here, and none of
   *  it goes through React: the `<svg>` is committed once, and the three
   *  properties a zoom touches are set by hand. A re-render in the middle of a
   *  pinch would otherwise have to be told to leave them alone. */
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
        // A correction, not a computed offset. `m-auto` centres a picture
        // smaller than the window and collapses once it is larger, and the
        // gutter sits outside the size the zoom computes, so an offset
        // derived from `scrollLeft` alone would fold both in and point
        // somewhere else the moment either changed — which is exactly when a
        // pinch carries the picture past the window's size. Measuring where
        // the anchored point *landed* is right whatever those did, and one
        // pass is enough because moving the scroll moves the picture by
        // exactly the amount taken out. Re-measured every frame of the ease,
        // so a scroll the browser clamped at the bounds simply corrects
        // itself on the next one.
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

  /** Ease what is painted towards `target`, one frame at a time, until it
   *  arrives. Time-based rather than a fixed fraction per frame so the feel is
   *  the same on a 60Hz display as on this machine's 120Hz one. */
  const run = useCallback(() => {
    if (raf.current != null) return;
    const tick = (ts: number) => {
      raf.current = null;
      const dt = stamp.current == null ? 16 : Math.min(64, ts - stamp.current);
      stamp.current = ts;
      const to = target.current;
      let z = current.current + (to - current.current) * (1 - Math.exp(-dt / SMOOTH_MS));
      // Land exactly, rather than approaching forever a tenth of a per cent
      // at a time with a repaint for each.
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

  /** Ask for a zoom, holding `(clientX, clientY)` over the same part of the
   *  diagram. Called with the window's centre when the zoom came from a button
   *  or a key, which is what "no cursor to anchor to" should mean.
   *
   *  Every zoom in the file goes through here, and every one of them composes
   *  on `target` rather than on what is painted — that is what lets a burst of
   *  wheel events, or four impatient clicks on +, add up instead of each
   *  overwriting the last. */
  const requestZoom = useCallback(
    (next: number, clientX?: number, clientY?: number) => {
      const el = scroller.current;
      const picture = host.current;
      // `painted`, not `current`: what is measured below belongs to the scale
      // on screen, and the smoother may already be a frame ahead of it.
      const now = painted.current;
      if (!el || !picture || now == null) return;
      const wanted = clampZoom(next);
      if (wanted === target.current) return;
      const view = el.getBoundingClientRect();
      const px = clientX ?? view.left + view.width / 2;
      const py = clientY ?? view.top + view.height / 2;
      const rect = picture.getBoundingClientRect();
      anchor.current = {
        // In the diagram's own coordinates, so it survives the scale change.
        x: (px - rect.left) / now,
        y: (py - rect.top) / now,
        // The screen position being held, which the correction measures against.
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

  // First paint: measure the window and fit to it. Layout, not effect — the
  // browser must never get a frame at a guessed scale.
  useLayoutEffect(() => {
    reset();
    // Only ever on open. `reset` is also the Fit button, and re-running this
    // because one of its deps was rebuilt would yank a zoomed-in reader back
    // to the corner.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // ── Zoom gestures ─────────────────────────────────────────────────────────
  // One pinch arrives twice over, and that duplication is a bug, not a
  // convenience: WKWebView synthesizes `ctrlKey` wheel events for the same
  // fingers that it reports through its own `gesture*` events, so answering
  // both applies the pinch twice. `gestureActive` picks one — while the
  // fingers are down the gesture owns the zoom, and the wheel path is left to
  // ⌘-scroll and to a real mouse. Plain scrolling is untouched: it is the pan.
  useEffect(() => {
    const el = scroller.current;
    if (!el) return;
    let pinchBase = 1;
    let gestureActive = false;
    let idle: number | null = null;

    // See GESTURE_IDLE_MS: the flag has to be able to clear itself.
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
      // `deltaMode === 1` counts lines rather than pixels, hence the 16px line.
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
      // `scale` is the magnification of the whole gesture so far, not a delta,
      // so this multiplies the zoom the pinch *started* from.
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

  // The window changing size under an open diagram moves what "fit" means.
  // Only the fit is re-measured — the picture is left where the reader put it.
  useEffect(() => {
    const el = scroller.current;
    if (!el || typeof ResizeObserver === "undefined") return;
    const ro = new ResizeObserver(() => {
      fit.current = fitZoom();
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, [fitZoom]);

  // ── Drag to pan ───────────────────────────────────────────────────────────
  // Pointer events, not HTML5 drag: a `dragstart` that sets no data is
  // cancelled outright by WebKit (root `CLAUDE.md`), and there is nothing
  // being dragged *to* here anyway — the gesture moves a viewport.
  const drag = useRef<{ x: number; y: number; left: number; top: number } | null>(null);
  const [dragging, setDragging] = useState(false);

  const onPointerDown = (e: React.PointerEvent) => {
    // Let a click on the toolbar, or a right-click, be what it is.
    if (e.button !== 0) return;
    const el = scroller.current;
    if (!el) return;
    // A label is text, and text is the one thing here that should answer to a
    // press by selecting rather than panning. Mermaid draws labels as SVG
    // `<text>`/`<tspan>` where it can (`htmlLabels: false` in `Mermaid.tsx`)
    // and as HTML inside a `<foreignObject>` for the diagram types with no
    // such switch, so both shapes are named. The browser's own selection is
    // better than any we would write.
    if ((e.target as Element).closest?.("text, tspan, foreignObject")) return;
    // WebKit starts a selection-drag from the first `pointermove` otherwise,
    // and the pan would drag a highlight across the diagram behind it.
    // Cancelling the pointerdown also cancels the focus it would have moved,
    // so the canvas takes focus by hand — the arrow keys and `+`/`-`/`0` are
    // its, and it is what `onOpenAutoFocus` hands the dialog to on open.
    e.preventDefault();
    el.focus();
    // A pan is a deliberate scroll, so an eased zoom still in flight must
    // stop correcting one: it would drag the picture back out from under the
    // hand every frame.
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

  // ── Keys ──────────────────────────────────────────────────────────────────
  // Escape is Radix's. The rest is what a picture viewer is expected to
  // answer to; arrows fall through to the scroller's own handling.
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

  // A double-click toggles between the whole picture and one detail of it,
  // which is the one thing a two-state gesture is good for. 100% is the
  // diagram at the size mermaid drew it — the size it is in a wide reply.
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
          // `overflow-scroll`, not `auto`. Under `auto` the scrollbars appear
          // and vanish as the picture crosses the window's size — and with
          // classic scrollbars on (`index.css`) that takes 15px out of
          // `clientWidth` mid-zoom, which re-centres `m-auto` and moves the
          // fit under the reader's fingers at exactly the moment they are
          // zooming through it. Reserved gutters cost two strips and make the
          // geometry constant. `scrollbar-gutter` would be the tidy way to say
          // this and is a no-op in WebKit.
          "flex flex-1 overflow-scroll outline-none",
          // The affordance has to match what the pointer will actually do:
          // an I-beam and a real selection over label text, the grab hand
          // everywhere else. SVG `<text>` is not selectable here by default at
          // all — `body` in `index.css` turns selection off app-wide — so
          // saying so is what makes "select when hovering over text" true.
          "[&_svg_text]:cursor-text [&_svg_text]:select-text",
          "[&_svg_foreignObject]:cursor-text [&_svg_foreignObject]:select-text",
          dragging
            ? // A pan must not paint a selection behind itself. The `!` is not
              // shouting: what these have to beat is the *pair of utilities
              // directly above*, which land in the same layer and are two
              // elements more specific — `.x svg text` against this rule's
              // `.x *` — so at equal weight the label keeps its I-beam and its
              // selection right through the pan. An important declaration is
              // what settles it. (It is not the cascade-layer trap recorded in
              // `index.css`: the base resets live inside `@layer base`, so a
              // plain `select-none` beats `span { user-select: text }` fine.)
              // `*` because the rule has to reach both kinds of label.
              "cursor-grabbing [&_*]:cursor-grabbing! [&_*]:select-none!"
            : "cursor-grab",
        )}
      >
        <div
          ref={box}
          // No `style` prop, deliberately: the size is written by `paint` and
          // React must have no opinion it could restore mid-pinch.
          //
          // `m-auto` is the centring, and it has to be an auto *margin* rather
          // than `justify-content: center` on the scroller: centred content
          // that outgrows its scroller overflows equally in both directions,
          // and the overflow before the start edge cannot be scrolled to — the
          // top-left corner of a zoomed-in diagram is simply unreachable. An
          // auto margin collapses to 0 once the free space goes negative, so
          // the same rule centres a small diagram and pins a large one.
          //
          // `shrink-0` is not tidying. This is a flex item, and a flex item's
          // default `flex-shrink: 1` squeezes it back to the container's width
          // — measured, the zoom climbed to 165% while the picture did not
          // move a pixel, because every extra pixel of width was being taken
          // straight back out.
          //
          // `box-content` puts the gutter outside the size the zoom computed,
          // so 100% means 100%. The deeper `pb` is the toolbar's room — see
          // `GUTTER_Y`.
          className="m-auto box-content shrink-0 p-9 pb-20"
        >
          <div
            ref={host}
            // Always the size mermaid drew, and scaled by a transform on top
            // — so the SVG lays out once, on open, and a zoom is a paint.
            // `transform-origin` at the corner is what keeps the host's
            // visual box flush with the layout box `paint` sized for it.
            style={{
              width: size.width,
              height: size.height,
              transformOrigin: "0 0",
              // Ask for the drawing to be its own compositing layer. The
              // transform then changes without the whole canvas being
              // re-rastered from scratch each frame, and WebKit still
              // re-rasters the vectors at the settled scale, so nothing ends
              // up soft.
              willChange: "transform",
            }}
            // The `!` on `max-w-none` is not tidying either: mermaid writes
            // its natural width as an **inline** `max-width`, and an inline
            // style outranks any class, so without the bang the picture stops
            // growing at natural size however far it is zoomed.
            className="[&>svg]:h-full [&>svg]:w-full [&>svg]:max-w-none!"
            dangerouslySetInnerHTML={{ __html: svg }}
          />
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

/** The controls, floating over the picture rather than in a bar above it —
 *  the window is the picture, and a bar would take a strip of it for a row
 *  that is empty most of the time. Same grammar as `PDFViewer`'s zoom cluster:
 *  ghost icon buttons around a percentage that resets when pressed, in
 *  `tabular-nums` so it does not jitter as the digits change.
 *
 *  The percentage arrives as a ref rather than a prop: it changes on every
 *  frame of a zoom, and a component that re-rendered for it would be the only
 *  React work in the gesture. */
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
          {/* No child in the JSX, deliberately. `paint` owns this text node,
              and a literal here would be restored the moment React re-rendered
              the toolbar for a `limit` change — which happens exactly when the
              zoom hits a clamp and stops painting, leaving "100%" frozen over
              a diagram at 800%. */}
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

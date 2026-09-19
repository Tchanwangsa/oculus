import { useEffect, useRef, useState } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";

/** A few pixels of slack around the target. A drag is aimed by hand at a
 *  strip of furniture, and a drop one pixel outside it reads as the app
 *  ignoring you rather than as a miss. */
const SLACK = 8;


/**
 * Files dropped onto one element, from Tauri's drag-and-drop events rather
 * than the page's.
 *
 * Tauri's own handler sits in front of the webview, so a file dragged in from
 * Finder never reaches a React `onDrop` — the page sees nothing at all.
 * Switching that handler off would hand file drops to WebKit and take in-page
 * dragging with it (CLAUDE.md), so Tauri's events are the ones to listen to.
 *
 * **Listen as the webview, not the window** — and in this app that is not the
 * choice the docs imply. A drop is delivered as a *window* event only when the
 * runtime built that webview as the window's own content
 * (`WebviewKind::WindowContent`); otherwise it is a *webview* event, emitted
 * to `EventTarget::Webview` — and Tauri's `filter_target` never matches a
 * `Window` listener against a `Webview` emit. This app turns on tauri's
 * `unstable` feature, because that is what `Window::add_child` needs for the
 * in-app browser (`app/src-tauri/src/browser.rs`), and that same feature flips
 * the *main* window's own webview to `WebviewKind::WindowChild`
 * (`tauri-runtime-wry`, where the kind is picked under `cfg(feature =
 * "unstable")`). So every drop here is webview-addressed, whatever the window
 * holds. A window listener subscribes without error, reports success, and is
 * then never called once — nothing to see but a drag that does nothing.
 *
 * **The position is in points, whatever its type says.** Tauri hands it over
 * as a `PhysicalPosition`, and it is not one: wry reads macOS's
 * `draggingLocation` and subtracts it from the view's frame height without
 * ever multiplying by the backing scale factor, so what arrives is the
 * window's own logical coordinates. Dividing those by `devicePixelRatio` — the
 * conversion the type asks for — halves every point on a retina screen, which
 * puts the whole window in its top-left quarter. So the scale is *measured*
 * instead of assumed: points to CSS pixels is the viewport's width over the
 * window's own logical width, which is `1` until the page is zoomed and stays
 * right when it is. It is re-read on a resize and again as each drag enters,
 * because page zoom changes both numbers under us.
 *
 * `over` is true while a drag is inside the element, for whatever the caller
 * wants to draw.
 */
export function useFileDrop(
  ref: React.RefObject<HTMLElement | null>,
  onDrop: (paths: string[]) => void,
) {
  const [over, setOver] = useState(false);
  // The handler is subscribed once; a fresh closure per render would
  // re-register the listener on every keystroke in the composer.
  const latest = useRef(onDrop);
  latest.current = onDrop;
  // Points → CSS pixels. 1 is the unzoomed answer, so an unmeasured drag is
  // right rather than merely close.
  const ratio = useRef(1);

  useEffect(() => {
    let dead = false;
    let unlisten: (() => void) | null = null;

    // Guarded because this is the one hook that reaches for the webview
    // itself: outside the app — the dev server opened in a plain browser —
    // there is nothing to ask, and an effect that throws takes the composer
    // down with it.
    let view: ReturnType<typeof getCurrentWebview>;
    try {
      view = getCurrentWebview();
    } catch {
      // No drop target here; paste still works.
      return;
    }

    const remeasure = () => {
      // `innerSize` is genuinely physical and `scaleFactor` is the screen's,
      // so their quotient is the window in the same points the drag reports.
      Promise.all([view.window.innerSize(), view.window.scaleFactor()])
        .then(([size, scale]) => {
          const points = size.width / (scale || 1);
          if (dead || points <= 0 || window.innerWidth <= 0) return;
          ratio.current = window.innerWidth / points;
        })
        .catch(() => {});
    };
    remeasure();
    window.addEventListener("resize", remeasure);

    const inside = (p: { x: number; y: number }) => {
      const el = ref.current;
      if (!el) return false;
      // **A background tab's box is still at these coordinates.** Panes are
      // hidden with `visibility`, never `display: none`, so that an unmounted
      // scroll position and a torn-down webview are not the price of switching
      // tabs (`app/src/components/tabs/TabPane.tsx`) — which leaves every
      // hidden composer holding a real rect under the visible one. Without
      // this, one drop lands in two boxes and the invisible one keeps the
      // picture until something sends it. `visibility` inherits, so asking the
      // element answers for the pane above it.
      if (getComputedStyle(el).visibility === "hidden") return false;
      const r = el.getBoundingClientRect();
      const x = p.x * ratio.current;
      const y = p.y * ratio.current;
      return (
        x >= r.left - SLACK &&
        x <= r.right + SLACK &&
        y >= r.top - SLACK &&
        y <= r.bottom + SLACK
      );
    };

    view
      .onDragDropEvent((e) => {
        const p = e.payload;
        if (p.type === "leave") {
          setOver(false);
          return;
        }
        if (p.type === "enter") {
          // The zoom may have changed since the last drag; there is a whole
          // hover's worth of events before the drop to land the answer in.
          remeasure();
          setOver(inside(p.position));
          return;
        }
        if (p.type === "over") {
          setOver(inside(p.position));
          return;
        }
        setOver(false);
        if (inside(p.position)) latest.current(p.paths);
      })
      .then((un) => {
        if (dead) un();
        else unlisten = un;
      })
      .catch(() => {});

    return () => {
      dead = true;
      window.removeEventListener("resize", remeasure);
      unlisten?.();
    };
  }, [ref]);

  return over;
}

/**
 * The kit's `Canvas`: react-three-fiber's, drawn only while the parent has
 * the frame in view (docs/widgets.md#a-3d-scene-releases-its-context-off-screen).
 * WebKit loses WebGL contexts past about sixteen live ones, so on `pause` the
 * scene is drawn once more into a PNG still that takes its place, the
 * renderer is disposed and its context dropped, and the r3f canvas unmounts;
 * `resume` mounts it again. Nothing is created before the first `resume`.
 * React state above the canvas survives a pause; state inside its tree
 * starts over, so keep it in `useWidgetState` or above. A context lost any
 * other way is reported as a runtime error and recreated on the next resume.
 * It also owns the label layer `Label3D` draws into.
 */
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Canvas as FiberCanvas, useFrame, type CanvasProps, type RootState } from "@react-three/fiber";
import { post } from "../../runtime/post";
import { getVisibility, subscribeVisibility } from "../../runtime/visibility";
import { cx } from "../../ui/cx";
import { LabelLayer } from "./LabelLayer";
import { LabelContext, LabelRegistry } from "./labels";

/** Canvases this module released on purpose, whose context loss is expected. */
const released = new WeakSet<HTMLCanvasElement>();

const LOST = "The 3D scene lost its WebGL context; it draws again when scrolled back into view";

/** Runs inside the scene: places the labels every frame, and on the first
 *  frame after a resume lets the still and the frozen labels go. */
function Frames({ labels, onFirst }: { labels: LabelRegistry; onFirst: () => void }) {
  const first = useRef(true);
  useFrame(({ camera, size }) => {
    if (first.current) {
      first.current = false;
      labels.thaw();
      onFirst();
    }
    labels.place(camera, size.width, size.height);
  });
  return null;
}

export function Canvas({ children, className, style, onCreated, resize, ...props }: CanvasProps) {
  const [drawing, setDrawing] = useState(() => getVisibility() === "live");
  const [still, setStill] = useState<string | null>(null);
  const root = useRef<RootState | null>(null);
  const labels = useMemo(() => new LabelRegistry(), []);

  const release = useCallback(() => {
    const state = root.current?.get();
    root.current = null;
    if (state) {
      labels.freeze();
      // Read in the same task as the render: no `preserveDrawingBuffer` needed.
      // PNG keeps the transparent background, so the card shows through.
      let url: string | null = null;
      try {
        state.gl.render(state.scene, state.camera);
        url = state.gl.domElement.toDataURL("image/png");
      } catch {
        url = null;
      }
      released.add(state.gl.domElement);
      state.gl.dispose();
      state.gl.forceContextLoss();
      if (url) setStill(url);
    }
    setDrawing(false);
  }, [labels]);

  useEffect(() => {
    const apply = () => {
      const visibility = getVisibility();
      if (visibility === "live") setDrawing(true);
      else if (visibility === "paused") release();
    };
    apply();
    return subscribeVisibility(apply);
  }, [release]);

  // Unmounted (a streaming preview replaced, the widget gone): the context
  // goes now and its loss is expected. r3f's own drop comes 500 ms later
  // and would read as a lost context.
  useEffect(
    () => () => {
      const state = root.current?.get();
      root.current = null;
      if (!state) return;
      released.add(state.gl.domElement);
      state.gl.dispose();
      state.gl.forceContextLoss();
    },
    [],
  );

  const created = useCallback(
    (state: RootState) => {
      root.current = state;
      const canvas = state.gl.domElement;
      canvas.addEventListener("webglcontextlost", () => {
        if (released.has(canvas)) return;
        post({ kind: "error", phase: "runtime", message: LOST });
        if (root.current === state) root.current = null;
        labels.freeze();
        setDrawing(false);
      });
      onCreated?.(state);
    },
    [labels, onCreated],
  );
  const drawn = useCallback(() => setStill(null), []);

  return (
    <div className={cx("o-canvas", className)} style={style}>
      {drawing && (
        <LabelContext.Provider value={labels}>
          <FiberCanvas
            {...props}
            // Offset sizes are the frame's own CSS px whatever the page zoom.
            resize={{ offsetSize: true, scroll: false, ...resize }}
            onCreated={created}
            style={{ position: "absolute", inset: 0 }}
          >
            {children}
            <Frames labels={labels} onFirst={drawn} />
          </FiberCanvas>
        </LabelContext.Provider>
      )}
      {still && <img className="o-canvas-still" src={still} alt="" draggable={false} />}
      <LabelLayer registry={labels} />
    </div>
  );
}

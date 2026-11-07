import { useLayoutEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { Canvas } from "./canvas/Canvas";
import type { Vec3 } from "./geometry/vec";
import { Rig, type View } from "./Rig";
import { DEFAULT_CAMERA, SceneScale, sceneScale } from "./scene";

export interface Scene3DProps {
  children?: ReactNode;
  height?: number;
  camera?: Vec3;
  target?: Vec3;
  orbit?: boolean;
  zoom?: boolean;
  pan?: boolean;
  autoRotate?: boolean;
}

export const DEFAULT_HEIGHT = 320;

/**
 * A 3D scene in maths coordinates (z up), the full width of its column and
 * `height` CSS px tall from the first frame, drawn on the kit's releasing
 * `Canvas`: transparent over the card, themed lights, orbiting about
 * `target`. Its box is always its full size, so drawing, pausing and
 * resuming never change the frame's height. No wheel zoom unless `zoom`.
 */
export function Scene3D({
  children,
  height = DEFAULT_HEIGHT,
  camera = DEFAULT_CAMERA,
  target = [0, 0, 0],
  orbit = true,
  zoom = false,
  pan = false,
  autoRotate = false,
}: Scene3DProps) {
  const ref = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(0);
  const view = useRef<View | null>(null);
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    // The column's own CSS px, whatever transform is around the frame.
    const measure = () => setWidth(el.clientWidth);
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(el);
    return () => observer.disconnect();
  }, []);
  // Where the view starts; after that the rig (and `view`) own the camera.
  const start = useRef(camera);
  const options = useMemo(() => ({ position: start.current, up: [0, 0, 1] as Vec3, fov: 45, near: 0.01, far: 1000 }), []);
  const dpr = typeof window === "undefined" ? 1 : Math.min(window.devicePixelRatio || 1, 2);
  return (
    <div ref={ref} className="o-scene" style={{ height }}>
      {width > 0 && (
        <SceneScale.Provider value={sceneScale(start.current, target)}>
          <Canvas camera={options} dpr={dpr} frameloop="always" gl={{ antialias: true, alpha: true }} flat>
            <Rig target={target} view={view} orbit={orbit} zoom={zoom} pan={pan} autoRotate={autoRotate} />
            {children}
          </Canvas>
        </SceneScale.Provider>
      )}
    </div>
  );
}

import { useEffect, useLayoutEffect, useRef, useState, type RefObject } from "react";
import { useFrame, useThree } from "@react-three/fiber";
import type { DirectionalLight } from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import type { Vec3 } from "./geometry/vec";

/** Where the student left the view: kept outside the canvas, so a pause
 *  (which unmounts it) doesn't put the camera back. */
export interface View {
  position: Vec3;
  target: Vec3;
}

/** Light that reads the same over light and dark cards: a fill everywhere,
 *  and a key from the camera so the faces turned to the student are lit. */
const AMBIENT = 1.4;
const KEY = 1.9;

/**
 * The scene's camera rig: orbit controls about `target` and the themed
 * lights. Zoom is off unless asked, so a wheel over the scene scrolls the
 * chat (OrbitControls only calls `preventDefault` on a wheel it zooms with).
 */
export function Rig({
  target,
  view,
  orbit,
  zoom,
  pan,
  autoRotate,
}: {
  target: Vec3;
  view: RefObject<View | null>;
  orbit: boolean;
  zoom: boolean;
  pan: boolean;
  autoRotate: boolean;
}) {
  const camera = useThree((s) => s.camera);
  const canvas = useThree((s) => s.gl.domElement);
  const light = useRef<DirectionalLight>(null);
  const [controls, setControls] = useState<OrbitControls | null>(null);
  const start = useRef(target);

  useLayoutEffect(() => {
    camera.up.set(0, 0, 1);
    const saved = view.current;
    if (saved) camera.position.set(...saved.position);
    const c = new OrbitControls(camera, canvas);
    c.target.set(...(saved?.target ?? start.current));
    c.enableDamping = true;
    c.update();
    const keep = () => {
      view.current = { position: camera.position.toArray() as Vec3, target: c.target.toArray() as Vec3 };
    };
    c.addEventListener("change", keep);
    setControls(c);
    return () => {
      c.removeEventListener("change", keep);
      c.dispose();
    };
  }, [camera, canvas, view]);

  useEffect(() => {
    if (!controls) return;
    controls.enableRotate = orbit;
    controls.enableZoom = zoom;
    controls.enablePan = pan;
    controls.autoRotate = autoRotate;
    // Touch scrolls the chat unless a gesture moves the view.
    canvas.style.touchAction = orbit || zoom || pan ? "none" : "auto";
  }, [controls, canvas, orbit, zoom, pan, autoRotate]);

  useFrame((_, delta) => {
    if (!controls) return;
    controls.update(delta);
    const key = light.current;
    if (!key) return;
    key.position.copy(camera.position);
    key.target.position.copy(controls.target);
    key.target.updateMatrixWorld();
  });

  return (
    <>
      <ambientLight intensity={AMBIENT} />
      <directionalLight ref={light} intensity={KEY} />
    </>
  );
}

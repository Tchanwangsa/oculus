import { useContext, useLayoutEffect, useRef } from "react";
import { paint, type Colour } from "../colors";
import { LabelContext, type Attach, type LabelSpec } from "./canvas/labels";
import type { Vec3 } from "./geometry/vec";

/**
 * Maths at a point of the scene, typeset by the kit's engine and drawn as DOM
 * over the canvas, so it always faces the student; centred on `at` or beside
 * it on the side `attach` names, hidden while `at` is behind the camera.
 * Draws nothing in the scene itself: it registers with its canvas's layer.
 */
export function Label3D({
  at,
  tex,
  children,
  attach,
  color,
}: {
  at: Vec3;
  tex?: string;
  children?: string | number;
  attach?: Attach;
  color?: Colour;
}) {
  const labels = useContext(LabelContext);
  if (!labels) throw new Error("Label3D goes inside a Scene3D or a Canvas");
  // Here, so a bad colour is this label's render error.
  paint(color, "foreground");
  const source = tex ?? (children == null ? "" : String(children));
  const spec = useRef<LabelSpec>({ at, source, attach, color });

  useLayoutEffect(() => {
    const s = spec.current;
    labels.add(s);
    return () => labels.remove(s);
  }, [labels]);

  useLayoutEffect(() => {
    const s = spec.current;
    s.at = at;
    if (s.source === source && s.attach === attach && s.color === color) return;
    Object.assign(s, { source, attach, color });
    labels.changed();
  });

  return null;
}

import { useEffect, useSyncExternalStore, type CSSProperties } from "react";
import { useTransformContext, vec } from "mafs";
import { loadTex, onTexReady, texEngine } from "../tex/load";
import { paint, type Colour } from "../colors";

type Vec = [number, number];

export type Attach = "n" | "ne" | "e" | "se" | "s" | "sw" | "w" | "nw";

/** The box the label is laid out in; it must hold any label a plane shows. */
const W = 640;
const H = 240;
/** CSS px between the point and an attached label. */
const GAP = 8;

const FLEX = { start: "flex-start", center: "center", end: "flex-end" } as const;

const load = () => void loadTex();

/**
 * Maths at a point of the plane, typeset by the kit's maths engine (the TeX
 * shows, muted, until it is in). Centred on `at`, or beside it on the side
 * `attach` names. Drawn as HTML in a `foreignObject`, fixed-position as Mafs
 * draws its own, which is what WebKit needs to place it inside the SVG.
 */
export function Label({
  at,
  tex,
  children,
  attach,
  color,
}: {
  at: Vec;
  tex?: string;
  children?: string | number;
  attach?: Attach;
  color?: Colour;
}) {
  const engine = useSyncExternalStore(onTexReady, texEngine, texEngine);
  useEffect(load, []);
  const { viewTransform, userTransform } = useTransformContext();
  const [px, py] = vec.transform(at, vec.matrixMult(viewTransform, userTransform));
  const source = tex ?? (children == null ? "" : String(children));

  const h = attach?.includes("e") ? "start" : attach?.includes("w") ? "end" : "center";
  const v = attach?.includes("n") ? "end" : attach?.includes("s") ? "start" : "center";
  const x = h === "start" ? px + GAP : h === "end" ? px - GAP - W : px - W / 2;
  const y = v === "start" ? py + GAP : v === "end" ? py - GAP - H : py - H / 2;

  let html: string | null = null;
  if (engine) {
    try {
      html = engine.renderTex(source, false);
    } catch {
      html = null;
    }
  }
  const box: CSSProperties = {
    position: "fixed",
    width: W,
    height: H,
    display: "flex",
    justifyContent: FLEX[h],
    alignItems: FLEX[v],
    color: paint(color, "foreground"),
  };
  return (
    <foreignObject x={x} y={y} width={W} height={H} pointerEvents="none" className="o-plane-label">
      <div style={box}>
        {html == null ? (
          <span className="o-tex o-tex-pending">{source}</span>
        ) : (
          <span className="o-tex" dangerouslySetInnerHTML={{ __html: html }} />
        )}
      </div>
    </foreignObject>
  );
}

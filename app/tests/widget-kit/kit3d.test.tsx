import { describe, expect, test } from "bun:test";
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import { renderToStaticMarkup } from "react-dom/server";
import * as THREE from "three";
import * as R3F from "@react-three/fiber";
import * as KitThree from "../../widget-kit/three";
import * as KitFiber from "../../widget-kit/fiber";
import * as Space from "../../widget-kit/3d";
import { Canvas } from "../../widget-kit/3d/canvas/Canvas";
import { LabelRegistry, type LabelSpec } from "../../widget-kit/3d/canvas/labels";
import { themeColour } from "../../widget-kit/3d/colour";
import { arcPoints } from "../../widget-kit/3d/geometry/arc";
import { arrowParts } from "../../widget-kit/3d/geometry/arrow";
import { curvePieces } from "../../widget-kit/3d/geometry/curve";
import { sphereRings } from "../../widget-kit/3d/geometry/sphere";
import { gridLines, surfaceGrid } from "../../widget-kit/3d/geometry/surface";
import { merge, tube } from "../../widget-kit/3d/geometry/tube";
import { length, sub, type Vec3 } from "../../widget-kit/3d/geometry/vec";
import { isHostMessage } from "../../widget-kit/runtime/host";
import { getVisibility, setVisibility, subscribeVisibility } from "../../widget-kit/runtime/visibility";

const KIT = join(import.meta.dir, "../../widget-kit");
const near = (a: Vec3, b: Vec3, eps = 1e-9) => length(sub(a, b)) < eps;

describe("three and fiber entries", () => {
  test("three is three, whole", () => {
    expect(KitThree.Vector3).toBe(THREE.Vector3);
    expect(Object.keys(KitThree).sort()).toEqual(Object.keys(THREE).sort());
  });

  test("fiber is r3f with the kit's releasing Canvas in place of its own", () => {
    expect(KitFiber.Canvas).toBe(Canvas);
    expect(KitFiber.Canvas).not.toBe(R3F.Canvas);
    expect(KitFiber.useFrame).toBe(R3F.useFrame);
    expect(Object.keys(KitFiber).sort()).toEqual(Object.keys(R3F).sort());
  });
});

describe("before the parent says resume", () => {
  test("Scene3D is its sized box with no canvas", () => {
    expect(getVisibility()).toBe("unknown");
    expect(renderToStaticMarkup(<Space.Scene3D />)).toBe('<div class="o-scene" style="height:320px"></div>');
    expect(renderToStaticMarkup(<Space.Scene3D height={240} />)).toContain("height:240px");
  });

  test("the kit's Canvas creates nothing", () => {
    const html = renderToStaticMarkup(<Canvas style={{ height: 200 }} />);
    expect(html).toBe('<div class="o-canvas" style="height:200px"></div>');
    expect(html).not.toContain("<canvas");
  });
});

describe("the frame's host messages", () => {
  test("the guard takes pause, resume and state", () => {
    expect(isHostMessage({ type: "oculus-host", kind: "pause" })).toBe(true);
    expect(isHostMessage({ type: "oculus-host", kind: "resume" })).toBe(true);
    expect(isHostMessage({ type: "oculus-host", kind: "state", state: { a: 1 } })).toBe(true);
  });

  test("and rejects anything else", () => {
    expect(isHostMessage({ type: "oculus-widget", kind: "pause" })).toBe(false);
    expect(isHostMessage({ type: "oculus-host", kind: "stop" })).toBe(false);
    expect(isHostMessage({ type: "oculus-host", kind: "state" })).toBe(false);
    expect(isHostMessage({ type: "oculus-host", kind: "state", state: [1] })).toBe(false);
    expect(isHostMessage({ type: "oculus-host", kind: "state", state: "x" })).toBe(false);
    expect(isHostMessage("pause")).toBe(false);
    expect(isHostMessage(null)).toBe(false);
  });

  test("the visibility store tells its listeners of each change once", () => {
    const seen: string[] = [];
    const off = subscribeVisibility(() => seen.push(getVisibility()));
    setVisibility("live");
    setVisibility("live");
    setVisibility("paused");
    off();
    setVisibility("live");
    expect(seen).toEqual(["live", "paused"]);
    setVisibility("unknown");
  });
});

describe("theme colours in 3D", () => {
  test("a hex or CSS name throws before any token is read", () => {
    expect(() => themeColour("#ff0000" as never, "brand")).toThrow(/"#ff0000" isn't a theme colour; use one of foreground/);
    expect(() => themeColour("red" as never, "brand")).toThrow(/"red" isn't a theme colour/);
  });

  test("no hex in the 3D kit or its stylesheet", () => {
    const files = (dir: string): string[] =>
      readdirSync(dir).flatMap((f) => (statSync(join(dir, f)).isDirectory() ? files(join(dir, f)) : [join(dir, f)]));
    for (const file of [...files(join(KIT, "3d")), join(KIT, "kit.css")])
      expect({ file, hex: readFileSync(file, "utf8").match(/#[0-9a-f]{3,8}\b|\b0x[0-9a-f]{3,8}\b/gi) }).toEqual({ file, hex: null });
  });
});

describe("3D geometry", () => {
  test("an arc runs the short way, on its radius, between its ends", () => {
    const pts = arcPoints([1, 1, 0], [2, 0, 0], [0, 0, 5]);
    expect(near(pts[0], [3, 1, 0])).toBe(true);
    expect(near(pts[pts.length - 1], [1, 1, 2])).toBe(true);
    for (const p of pts) expect(length(sub(p, [1, 1, 0]))).toBeCloseTo(2, 9);
    // A quarter turn, so every point is in the first quadrant of its plane.
    for (const p of pts) expect(p[0] >= 1 - 1e-9 && p[2] >= -1e-9).toBe(true);
    const r = arcPoints([0, 0, 0], [1, 0, 0], [1, 1, 0], 3);
    expect(near(r[r.length - 1], [3 / Math.SQRT2, 3 / Math.SQRT2, 0])).toBe(true);
  });

  test("opposite directions turn about z (x when vertical) without NaN; the same direction is no arc", () => {
    const flat = arcPoints([0, 0, 0], [1, 0, 0], [-1, 0, 0]);
    expect(flat.every((p) => p.every(Number.isFinite))).toBe(true);
    expect(near(flat[flat.length - 1], [-1, 0, 0], 1e-9)).toBe(true);
    expect(Math.max(...flat.map((p) => Math.abs(p[2])))).toBeCloseTo(0, 9);
    const polar = arcPoints([0, 0, 0], [0, 0, 1], [0, 0, -1]);
    expect(polar.every((p) => p.every(Number.isFinite))).toBe(true);
    expect(near(polar[Math.floor(polar.length / 2)], [0, 1, 0], 1e-9)).toBe(true);
    expect(arcPoints([0, 0, 0], [1, 0, 0], [2, 0, 0])).toEqual([]);
    expect(arcPoints([0, 0, 0], [0, 0, 0], [1, 0, 0])).toEqual([]);
  });

  test("a curve breaks where its points aren't finite", () => {
    const pieces = curvePieces((t) => [t, 1 / (t - 0.5), 0], [0, 1], 10);
    expect(pieces.length).toBe(2);
    expect(pieces[0].length + pieces[1].length).toBe(10);
    expect(pieces.flat().every((p) => p.every(Number.isFinite))).toBe(true);
    // A lone point between two gaps draws nothing.
    expect(curvePieces((t) => [t, t === 0.5 ? 0 : NaN, 0], [0, 1], 2)).toEqual([]);
    expect(curvePieces((t) => [Math.cos(t), Math.sin(t), t], [0, 1], 4).length).toBe(1);
  });

  test("a surface has a vertex per sample and two triangles per whole cell", () => {
    const g = surfaceGrid((x, y) => [x, y, x * y], [0, 1], [0, 1], 4);
    expect(g.positions.length).toBe(25 * 3);
    expect(g.indices.length).toBe(16 * 6);
    // A hole at the centre sample takes the four cells around it.
    const holed = surfaceGrid((x, y) => [x, y, x === 0.5 && y === 0.5 ? NaN : 0], [0, 1], [0, 1], 4);
    expect(holed.indices.length).toBe(12 * 6);
    expect(holed.real.filter((r) => !r).length).toBe(1);
    expect([...holed.positions].every(Number.isFinite)).toBe(true);
    // Wire lines along u and v break at the hole.
    expect(gridLines(holed, 4, 2).length).toBe(8);
  });

  test("a wire sphere's rings lie on it", () => {
    const rings = sphereRings([1, 0, 0], 2, 5, 12);
    expect(rings.length).toBe(17);
    for (const ring of rings) for (const p of ring) expect(length(sub(p, [1, 0, 0]))).toBeCloseTo(2, 9);
    expect(near(rings[0][0], rings[0][rings[0].length - 1])).toBe(true);
    // The middle latitude is the equator; a meridian runs pole to pole.
    expect(rings[2][0][2]).toBeCloseTo(0, 9);
    expect(near(rings[5][0], [1, 0, 2])).toBe(true);
    expect(near(rings[5][rings[5].length - 1], [1, 0, -2])).toBe(true);
  });

  test("a tube has a ring per point and caps unless it closes", () => {
    const open = tube([[0, 0, 0], [1, 0, 0], [1, 1, 0]], 0.1, 8);
    expect(open.positions.length / 3).toBe(3 * 8 + 2 * 9);
    expect(open.indices.length).toBe(2 * 8 * 6 + 2 * 8 * 3);
    const ring = sphereRings([0, 0, 0], 1, 1, 0)[0];
    const closed = tube(ring, 0.1, 8);
    expect(closed.positions.length / 3).toBe((ring.length - 1) * 8);
    expect(closed.indices.length).toBe((ring.length - 1) * 8 * 6);
    expect([...open.positions, ...open.normals].every(Number.isFinite)).toBe(true);
    expect(tube([[1, 1, 1], [1, 1, 1]], 0.1).indices.length).toBe(0);
    const both = merge([open, closed]);
    expect(both.indices.length).toBe(open.indices.length + closed.indices.length);
    expect(Math.max(...both.indices)).toBe(both.positions.length / 3 - 1);
  });

  test("an arrow's head keeps its share when short and its size when long", () => {
    const short = arrowParts([0, 0, 0], [0, 0, 0.5], 0.2)!;
    expect(short.headLength).toBeCloseTo(0.15, 9);
    const long = arrowParts([0, 0, 0], [0, 0, 10], 0.2)!;
    expect(long.headLength).toBe(0.2);
    expect(near(long.neck, [0, 0, 9.8])).toBe(true);
    expect(arrowParts([1, 2, 3], [1, 2, 3], 0.2)).toBeNull();
  });
});

describe("3D labels", () => {
  const element = () => ({ style: {} as Record<string, string> }) as unknown as HTMLElement;
  const camera = () => {
    const c = new THREE.PerspectiveCamera(45, 2, 0.1, 100);
    c.up.set(0, 0, 1);
    c.position.set(5, 0, 0);
    c.lookAt(0, 0, 0);
    return c;
  };

  test("a label is placed where its point projects, and hidden behind the camera", () => {
    const labels = new LabelRegistry();
    const front: LabelSpec = { at: [0, 0, 0], source: "O" };
    const behind: LabelSpec = { at: [10, 0, 0], source: "B" };
    labels.add(front);
    labels.add(behind);
    const a = element();
    const b = element();
    labels.bind(0, a);
    labels.bind(1, b);
    labels.place(camera(), 400, 200);
    const [x, y] = (a.style.transform.match(/^translate\((.+)px, (.+)px\)$/) ?? []).slice(1).map(Number);
    expect(x).toBeCloseTo(200, 6);
    expect(y).toBeCloseTo(100, 6);
    expect(a.style.visibility).toBe("visible");
    expect(b.style.visibility).toBe("hidden");
  });

  test("frozen labels stay until the scene draws again", () => {
    const labels = new LabelRegistry();
    const spec: LabelSpec = { at: [0, 0, 1], source: "z" };
    let notes = 0;
    labels.subscribe(() => notes++);
    labels.add(spec);
    expect(labels.snapshot()).toEqual([spec]);
    labels.freeze();
    labels.remove(spec);
    expect(labels.snapshot()).toEqual([spec]);
    const again: LabelSpec = { at: [0, 0, 1], source: "z" };
    labels.add(again);
    expect(labels.snapshot()).toEqual([spec]);
    labels.thaw();
    expect(labels.snapshot()[0]).toBe(again);
    expect(notes).toBe(2);
  });

  test("Label3D outside a scene says where it goes", () => {
    expect(() => renderToStaticMarkup(<Space.Label3D at={[0, 0, 0]} tex="x" />)).toThrow(/Label3D goes inside a Scene3D or a Canvas/);
  });
});

// title: Following a vector field
import { Row, Select, Value, Widget } from "oculus/ui";
import { useWidgetState } from "oculus/state";
import { MovablePoint, Plane, Plot, Vector, type Vec } from "oculus/2d";

const FIELDS: Record<string, { label: string; at: (p: Vec) => Vec }> = {
  rotation: { label: "Rotation (−y, x)", at: ([x, y]) => [-y, x] },
  source: { label: "Source (x, y)", at: ([x, y]) => [x, y] },
  saddle: { label: "Saddle (x, −y)", at: ([x, y]) => [x, -y] },
  shear: { label: "Shear (y, 0)", at: ([, y]) => [y, 0] },
};
const OPTIONS = Object.entries(FIELDS).map(([value, f]) => ({ value, label: f.label }));
const SPAN = 4;
const GRID: Vec[] = [];
for (let x = -SPAN; x <= SPAN; x++) for (let y = -SPAN; y <= SPAN; y++) GRID.push([x, y]);

export default function VectorField() {
  const [kind, setKind] = useWidgetState("kind", "rotation");
  const [start, setStart] = useWidgetState<Vec>("start", [2, 0]);
  const field = (FIELDS[kind] ?? FIELDS.rotation).at;

  // A field is many short arrows: scale the longest to fit one grid cell and
  // let opacity carry the strength, so colour stays free for meaning.
  const longest = Math.max(...GRID.map((p) => Math.hypot(...field(p))), 1e-9);
  const path = trace(field, start);
  const { div, curl } = local(field, start);

  return (
    <Widget>
      <Plane x={[-SPAN, SPAN]} y={[-SPAN, SPAN]} height={300}>
        {GRID.map(([x, y]) => {
          const [u, v] = field([x, y]);
          const s = 0.8 / longest;
          const strength = Math.hypot(u, v) / longest;
          return <Vector key={`${x},${y}`} tail={[x, y]} tip={[x + u * s, y + v * s]} color="muted" opacity={0.25 + 0.75 * strength} />;
        })}
        <Plot
          xy={(t) => path[Math.min(path.length - 1, Math.round(t))]}
          domain={[0, path.length - 1]}
          color="brand"
          weight={2.5}
        />
        <MovablePoint point={start} onMove={setStart} />
      </Plane>
      <Row justify="between">
        <Select label="Field" value={kind} options={OPTIONS} onChange={setKind} />
        <Row gap={16}>
          <Value label="Divergence" value={div} digits={2} />
          <Value label="Curl" value={curl} digits={2} />
        </Row>
      </Row>
    </Widget>
  );
}

/** The path a particle dropped at `start` takes, by small Euler steps. */
function trace(at: (p: Vec) => Vec, start: Vec): Vec[] {
  const path: Vec[] = [start];
  let [x, y] = start;
  for (let i = 0; i < 400 && Math.abs(x) < SPAN + 1 && Math.abs(y) < SPAN + 1; i++) {
    const [u, v] = at([x, y]);
    x += u * 0.02;
    y += v * 0.02;
    path.push([x, y]);
  }
  return path;
}

/** Divergence and curl at `p`, by central differences. */
function local(at: (p: Vec) => Vec, [x, y]: Vec) {
  const h = 1e-3;
  const dudx = (at([x + h, y])[0] - at([x - h, y])[0]) / (2 * h);
  const dvdy = (at([x, y + h])[1] - at([x, y - h])[1]) / (2 * h);
  const dvdx = (at([x + h, y])[1] - at([x - h, y])[1]) / (2 * h);
  const dudy = (at([x, y + h])[0] - at([x, y - h])[0]) / (2 * h);
  return { div: dudx + dvdy, curl: dvdx - dudy };
}

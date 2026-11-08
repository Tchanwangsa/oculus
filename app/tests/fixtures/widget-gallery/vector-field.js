// title: Following a vector field
import { Row, Select, Value, Widget } from "oculus/ui";
import { useWidgetState } from "oculus/state";
import { MovablePoint, Plane, Plot, Vector } from "oculus/2d";
import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
const FIELDS = {
	rotation: {
		label: "Rotation (−y, x)",
		at: ([x, y]) => [-y, x]
	},
	source: {
		label: "Source (x, y)",
		at: ([x, y]) => [x, y]
	},
	saddle: {
		label: "Saddle (x, −y)",
		at: ([x, y]) => [x, -y]
	},
	shear: {
		label: "Shear (y, 0)",
		at: ([, y]) => [y, 0]
	}
};
const OPTIONS = Object.entries(FIELDS).map(([value, f]) => ({
	value,
	label: f.label
}));
const SPAN = 4;
const GRID = [];
for (let x = -SPAN; x <= SPAN; x++) for (let y = -SPAN; y <= SPAN; y++) GRID.push([x, y]);
function VectorField() {
	const [kind, setKind] = useWidgetState("kind", "rotation");
	const [start, setStart] = useWidgetState("start", [2, 0]);
	const field = (FIELDS[kind] ?? FIELDS.rotation).at;
	// A field is many short arrows: scale the longest to fit one grid cell and
	// let opacity carry the strength, so colour stays free for meaning.
	const longest = Math.max(...GRID.map((p) => Math.hypot(...field(p))), 1e-9);
	const path = trace(field, start);
	const { div, curl } = local(field, start);
	return _jsxs(Widget, { children: [_jsxs(Plane, {
		x: [-SPAN, SPAN],
		y: [-SPAN, SPAN],
		height: 300,
		children: [
			GRID.map(([x, y]) => {
				const [u, v] = field([x, y]);
				const s = .8 / longest;
				const strength = Math.hypot(u, v) / longest;
				return _jsx(Vector, {
					tail: [x, y],
					tip: [x + u * s, y + v * s],
					color: "muted",
					opacity: .25 + .75 * strength
				}, `${x},${y}`);
			}),
			_jsx(Plot, {
				xy: (t) => path[Math.min(path.length - 1, Math.round(t))],
				domain: [0, path.length - 1],
				color: "brand",
				weight: 2.5
			}),
			_jsx(MovablePoint, {
				point: start,
				onMove: setStart
			})
		]
	}), _jsxs(Row, {
		justify: "between",
		children: [_jsx(Select, {
			label: "Field",
			value: kind,
			options: OPTIONS,
			onChange: setKind
		}), _jsxs(Row, {
			gap: 16,
			children: [_jsx(Value, {
				label: "Divergence",
				value: div,
				digits: 2
			}), _jsx(Value, {
				label: "Curl",
				value: curl,
				digits: 2
			})]
		})]
	})] });
}
/** The path a particle dropped at `start` takes, by small Euler steps. */
function trace(at, start) {
	const path = [start];
	let [x, y] = start;
	for (let i = 0; i < 400 && Math.abs(x) < SPAN + 1 && Math.abs(y) < SPAN + 1; i++) {
		const [u, v] = at([x, y]);
		x += u * .02;
		y += v * .02;
		path.push([x, y]);
	}
	return path;
}
/** Divergence and curl at `p`, by central differences. */
function local(at, [x, y]) {
	const h = .001;
	const dudx = (at([x + h, y])[0] - at([x - h, y])[0]) / (2 * h);
	const dvdy = (at([x, y + h])[1] - at([x, y - h])[1]) / (2 * h);
	const dvdx = (at([x + h, y])[1] - at([x - h, y])[1]) / (2 * h);
	const dudy = (at([x, y + h])[0] - at([x, y - h])[0]) / (2 * h);
	return {
		div: dudx + dvdy,
		curl: dvdx - dudy
	};
}

const __oculusWidget = VectorField;

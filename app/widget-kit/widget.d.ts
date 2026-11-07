/**
 * What a ```widget fence may import besides `react` (docs/widgets.md). A
 * widget is a TSX module whose default export is a component taking no
 * props; its first line is `// title: …`. Imports other than `react`,
 * `oculus/ui`, `oculus/state`, `oculus/2d`, `oculus/math`, `oculus/3d`,
 * `oculus/ask`, `three` and `@react-three/fiber` are refused before it runs
 * (no `three/addons/…`, no drei). `oculus/2d`, `oculus/math`, `oculus/ask`
 * and the 3D modules load only when imported.
 *
 * The frame has no network. `oculus/ask` is the one way out: a billed model
 * call, or a message into the chat, each only after the student allows it.
 * Use it only when the widget can't work without a model.
 *
 * `three` is three.js 0.186 and `@react-three/fiber` is r3f 9.8, as
 * published (their types are theirs, not declared here), with one change:
 * r3f's `Canvas` is the kit's, which draws only while the widget is in view
 * and swaps in a still image otherwise, as `Scene3D` does; it is 320px tall
 * unless its `style` sets a height. State inside a `Canvas` starts over when
 * it comes back into view; keep what must last in `useWidgetState` or above
 * the `Canvas`. A renderer made by hand (`new WebGLRenderer()`) is never
 * released and can lose its context.
 *
 * The frame is exactly as tall as the widget, with no scrollbar: keep it
 * under 560px and put longer content on `Pages`. Colours and fonts come from
 * the app's theme; don't set your own.
 *
 * Contents, in order:
 * - `oculus/ui`: `Widget`, `Row`, `Column`, `Pages`, `Tabs`, `Slider`,
 *   `Toggle`, `Select`, `Field`, `Pill`, `Value`, `Tex`, `Note`
 * - `oculus/state`: `useWidgetState`
 * - `oculus/2d`: `Colour`, `Plane`, `Point`, `MovablePoint`, `Vector`,
 *   `Line`, `Segment`, `Circle`, `Arc`, `Angle`, `Polygon`, `Plot`, `Label`
 * - `oculus/math`: `parse`, `compile`, `evaluate`, `derivative`, `simplify`,
 *   complex numbers, matrices, arithmetic, `math`, `formula`
 * - `oculus/3d`: `Scene3D`, `Axes3D`, `Sphere`, `Arrow3D`, `Arc3D`,
 *   `Curve3D`, `Surface`, `Label3D`
 * - `oculus/ask`: `ask`, `send`, `useAsk`
 */

declare module "oculus/ui" {
  import type { ReactNode } from "react";

  type Align = "start" | "center" | "end" | "stretch" | "baseline";

  /** The root: the card's padding and a column of the widget's parts. */
  export function Widget(props: { children?: ReactNode; gap?: number }): ReactNode;

  /** Side by side; wraps onto the next line when the frame is narrow unless `wrap` is false. */
  export function Row(props: {
    children?: ReactNode;
    gap?: number;
    align?: Align;
    justify?: "start" | "center" | "end" | "between";
    wrap?: boolean;
  }): ReactNode;

  /** Stacked. `flex` shares a `Row` with its siblings. */
  export function Column(props: { children?: ReactNode; gap?: number; align?: Align; flex?: number }): ReactNode;

  /**
   * One child per page, with Back/Next and a dot per page. The frame is the
   * tallest page's height from the start, so paging never resizes it.
   * Uncontrolled, or controlled with `page`/`onPageChange`.
   */
  export function Pages(props: { children?: ReactNode; page?: number; onPageChange?: (page: number) => void }): ReactNode;

  /** One tab per child, in `labels` order; as tall as the tallest panel. */
  export function Tabs(props: {
    labels: string[];
    children?: ReactNode;
    value?: number;
    onChange?: (index: number) => void;
  }): ReactNode;

  /** A labelled range with its value shown (tabular figures). */
  export function Slider(props: {
    label?: ReactNode;
    value: number;
    onChange: (value: number) => void;
    min?: number;
    max?: number;
    step?: number;
    unit?: string;
    digits?: number;
  }): ReactNode;

  /** An on/off switch. */
  export function Toggle(props: {
    label?: ReactNode;
    checked: boolean;
    onChange: (checked: boolean) => void;
    disabled?: boolean;
  }): ReactNode;

  export type Option = string | { value: string; label: string };

  /** One choice from a list. */
  export function Select(props: {
    label?: ReactNode;
    value: string;
    options: Option[];
    onChange: (value: string) => void;
  }): ReactNode;

  /** A text input; `formula` treats the value as TeX and draws it under the field. Never focuses itself. */
  export function Field(props: {
    label?: ReactNode;
    value: string;
    onChange: (value: string) => void;
    placeholder?: string;
    formula?: boolean;
    hint?: ReactNode;
  }): ReactNode;

  /** A small pill button. */
  export function Pill(props: {
    children?: ReactNode;
    onClick?: () => void;
    tone?: "outline" | "primary" | "ghost";
    pressed?: boolean;
    disabled?: boolean;
  }): ReactNode;

  /** A labelled number; `digits` fixes the decimals. */
  export function Value(props: { label?: ReactNode; value: number | string; unit?: string; digits?: number }): ReactNode;

  /**
   * Maths as TeX: `<Tex>{"\\frac{a}{b}"}</Tex>` or `tex={"\\frac{a}{b}"}`;
   * `display` sets it on its own line. Not `tex="\\frac…"`: a JSX attribute
   * string keeps both backslashes.
   */
  export function Tex(props: { children?: string | number; tex?: string; display?: boolean }): ReactNode;

  /** A short line of explanation. */
  export function Note(props: { children?: ReactNode; tone?: "muted" | "success" | "warning" | "danger" }): ReactNode;
}

declare module "oculus/state" {
  /**
   * Like `useState`, but kept with the reply: it survives reloads, theme
   * changes and the expanded view. Values must be JSON.
   */
  export function useWidgetState<T>(key: string, initial: T): [T, (next: T | ((prev: T) => T)) => void];
}

declare module "oculus/2d" {
  import type { ReactNode } from "react";

  /** `[x, y]` in the plane's maths coordinates. */
  export type Vec = [number, number];

  /**
   * A colour of the app's theme, light or dark. Nothing else is accepted: a
   * hex or CSS colour name is an error. `chart-1`…`chart-5` are the series
   * colours, in the order that keeps them apart; `muted` is for guides.
   */
  export type Colour =
    | "foreground"
    | "muted"
    | "brand"
    | "success"
    | "warning"
    | "danger"
    | "chart-1"
    | "chart-2"
    | "chart-3"
    | "chart-4"
    | "chart-5";

  export interface Stroke {
    color?: Colour;
    /** Stroke width in CSS px. */
    weight?: number;
    style?: "solid" | "dashed";
    opacity?: number;
  }

  export interface Fill extends Stroke {
    /** 0 for an outline only. */
    fillOpacity?: number;
  }

  /**
   * The plane: axes, a grid and labelled ticks, in maths coordinates (y up),
   * the full width of its column and `height` CSS px tall (default 320). The
   * `x` and `y` ranges (default -5…5) are always in view with a small margin;
   * with `aspect="equal"` (default) one unit is as long on both axes, so the
   * longer side shows more, and `"fill"` stretches the ranges to the frame.
   * `step` spaces grid lines and ticks (default 1, or π/2 on x with
   * `labels="pi"`); `grid="none"` keeps the axes and ticks only. Wheel and
   * pinch never zoom it; `pan` lets a drag move the view. `onClick` gets the
   * maths point clicked. Everything else in this module draws inside it.
   */
  export function Plane(props: {
    children?: ReactNode;
    x?: [min: number, max: number];
    y?: [min: number, max: number];
    height?: number;
    grid?: "cartesian" | "polar" | "none";
    step?: number | [x: number, y: number];
    subdivisions?: number;
    labels?: boolean | "pi";
    axes?: boolean;
    aspect?: "equal" | "fill";
    pan?: boolean;
    onClick?: (point: Vec) => void;
  }): ReactNode;

  /** A dot at `(x, y)` (default colour `foreground`). */
  export function Point(props: { x: number; y: number; color?: Colour; opacity?: number }): ReactNode;

  /**
   * A point the student drags, or moves with the arrow keys once focused
   * (default colour `brand`). Controlled: keep `point` in state and set it
   * from `onMove`. `constrain` keeps it on a horizontal or vertical line, or
   * maps the pointer's position to where the point may go (onto a curve, a
   * circle, a grid).
   */
  export function MovablePoint(props: {
    point: Vec;
    onMove: (point: Vec) => void;
    constrain?: "horizontal" | "vertical" | ((point: Vec) => Vec);
    color?: Colour;
  }): ReactNode;

  /** An arrow from `tail` (default the origin) to `tip`. */
  export function Vector(props: Stroke & { tail?: Vec; tip: Vec }): ReactNode;

  /** An infinite line through two points, or through `point` with a `slope`
   *  or an `angle` (radians from the x-axis). */
  export function Line(
    props: Stroke & ({ point1: Vec; point2: Vec } | { point: Vec; slope: number } | { point: Vec; angle: number }),
  ): ReactNode;

  /** The segment between two points. */
  export function Segment(props: Stroke & { point1: Vec; point2: Vec }): ReactNode;

  /** A circle of `radius` about `center` (default colour `chart-1`). */
  export function Circle(props: Fill & { center: Vec; radius: number }): ReactNode;

  /**
   * Part of a circle of `radius` about `center`, from angle `from` to `to`
   * (radians, counterclockwise from the x-axis; `to` below `from` runs
   * clockwise).
   */
  export function Arc(props: Stroke & { center: Vec; radius: number; from: number; to: number }): ReactNode;

  /**
   * The angle at `vertex` between the rays to `from` and to `to`: a small
   * wedge `radius` CSS px across (default 26), a square corner when it is
   * right. The smaller angle, unless `directed`, which runs counterclockwise
   * from `from` to `to` and may pass a half turn. `label` is TeX set just
   * outside the wedge.
   */
  export function Angle(props: {
    vertex: Vec;
    from: Vec;
    to: Vec;
    radius?: number;
    directed?: boolean;
    label?: string;
    color?: Colour;
    weight?: number;
    fillOpacity?: number;
  }): ReactNode;

  /** A closed polygon through `points`, in order (default colour `chart-1`). */
  export function Polygon(props: Fill & { points: Vec[] }): ReactNode;

  /**
   * A curve (default colour `chart-1`): `y={(x) => …}` across the plane or
   * over `domain`; `x={(y) => …}`; `xy={(t) => [x, y]}` over `domain`; or
   * polar `r={(θ) => …}` over `domain` (default one turn). Points that aren't
   * finite are left out. For a formula the student types, build the function
   * with `formula` from `oculus/math`.
   */
  export function Plot(
    props: Stroke &
      (
        | { y: (x: number) => number; domain?: [min: number, max: number] }
        | { x: (y: number) => number; domain?: [min: number, max: number] }
        | { xy: (t: number) => Vec; domain: [min: number, max: number] }
        | { r: (theta: number) => number; domain?: [min: number, max: number] }
      ),
  ): ReactNode;

  /**
   * Maths at a point, as TeX: `<Label at={[1, 2]} tex="P_1" />` or
   * `<Label at={p}>{"\\theta"}</Label>`. Centred on `at`, or beside it on the
   * side `attach` names (`"ne"` is above and to the right).
   */
  export function Label(props: {
    at: Vec;
    tex?: string;
    children?: string | number;
    attach?: "n" | "ne" | "e" | "se" | "s" | "sw" | "w" | "nw";
    color?: Colour;
  }): ReactNode;
}

declare module "oculus/math" {
  /**
   * mathjs, a subset: parsing and evaluating expressions, derivatives,
   * simplification, complex numbers and matrices. Expressions use mathjs
   * syntax (`2x^2 + sin(t)`, `[1, 2; 3, 4]`, `3 + 2i`). Every call here works
   * in the widget's sandbox. Functions beyond the list below (`gamma`,
   * `lusolve`, units, big numbers) are not in the kit.
   */

  export interface Complex {
    re: number;
    im: number;
    toString(): string;
  }
  export interface Matrix {
    size(): number[];
    toArray(): MathValue[];
    valueOf(): MathValue[];
  }
  export type MathValue = number | Complex | Matrix | MathValue[];
  export type Scope = Record<string, unknown>;

  /**
   * A parsed expression. `toString()` is mathjs syntax, `toTex()` is TeX for
   * `Tex`. `type` says what it is: an `OperatorNode` has `op` (`"+"`, `"*"`,
   * `"^"`, …) and its operands in `args`; a `FunctionNode` has `name`
   * (`"sin"`) and `args`; a `ParenthesisNode` holds `content`; a
   * `SymbolNode` has `name`. Reading these is how a widget says which rule
   * an expression calls for.
   */
  export interface Node {
    type: "OperatorNode" | "FunctionNode" | "ParenthesisNode" | "SymbolNode" | "ConstantNode" | (string & {});
    op?: string;
    name?: string;
    args?: Node[];
    content?: Node;
    toString(): string;
    toTex(): string;
    evaluate(scope?: Scope): any;
    compile(): Compiled;
  }
  export interface Compiled {
    evaluate(scope?: Scope): any;
  }

  export function parse(expression: string): Node;
  export function compile(expression: string): Compiled;
  /** `evaluate("x^2", { x: 3 })` is 9. */
  export function evaluate(expression: string, scope?: Scope): any;
  /** `derivative("x^2 * sin(x)", "x").toString()` is `2 * x * sin(x) + x ^ 2 * cos(x)`. */
  export function derivative(expression: string | Node, variable: string, options?: { simplify?: boolean }): Node;
  /** `simplify("2x + 3x").toString()` is `5 * x`. */
  export function simplify(expression: string | Node, scope?: Scope): Node;

  export function complex(re: number, im?: number): Complex;
  export function complex(text: string): Complex;
  export function matrix(data: MathValue[]): Matrix;
  /** Eigenvalues, and each eigenvalue with its eigenvector. */
  export function eigs(m: MathValue[] | Matrix): {
    values: MathValue[] | Matrix;
    eigenvectors: { value: number | Complex; vector: MathValue[] | Matrix }[];
  };
  export function det(m: MathValue[] | Matrix): number;
  export function inv<T extends MathValue>(m: T): T;
  export function transpose<T extends MathValue>(m: T): T;
  export function identity(n: number): Matrix;
  export function zeros(rows: number, columns: number): Matrix;
  export function dot(a: MathValue, b: MathValue): number;
  export function cross(a: MathValue, b: MathValue): MathValue;
  export function norm(a: MathValue): number;

  /** Arithmetic over numbers, complex numbers and matrices. */
  export function add(a: MathValue, b: MathValue): any;
  export function subtract(a: MathValue, b: MathValue): any;
  export function multiply(a: MathValue, b: MathValue): any;
  export function divide(a: MathValue, b: MathValue): any;
  export function pow(a: MathValue, b: MathValue): any;
  /** `sqrt(-4)` is the complex `2i`. */
  export function sqrt(x: number | Complex): number | Complex;
  export function exp(x: number | Complex): number | Complex;
  export function log(x: number | Complex, base?: number): number | Complex;
  export function sin(x: number | Complex): number | Complex;
  export function cos(x: number | Complex): number | Complex;
  export function tan(x: number | Complex): number | Complex;
  export function atan2(y: number, x: number): number;
  export function abs(x: number | Complex): number;
  export function re(x: number | Complex): number;
  export function im(x: number | Complex): number;
  export function arg(x: number | Complex): number;
  export function conj(x: number | Complex): number | Complex;
  export const pi: number;
  export const e: number;
  export const i: Complex;

  /**
   * The whole instance, for what the named exports leave out. Expressions
   * may also call: sign, floor, ceil, round, mod, min, max, cbrt, nthRoot,
   * log10, log2, sec, csc, cot, asin, acos, atan, asec, acsc, acot, sinh,
   * cosh, tanh, sech, csch, coth, asinh, acosh, atanh; and name tau.
   */
  export const math: Record<string, any>;

  export type Formula =
    | {
        ok: true;
        /** The formula of the variables, in their order. A value that isn't a
         *  real number (complex, a matrix, a domain error) is `NaN`. */
        fn: (...values: number[]) => number;
        /** The formula as TeX, for `Tex`. */
        tex: string;
      }
    | { ok: false; error: string };

  /**
   * A formula someone typed as a function of `variables` (default `["x"]`):
   * `formula("a sin(x)", ["x", "a"])`. Never throws. A formula that doesn't
   * parse, or names something that is neither a variable, a constant nor a
   * function above, is `{ ok: false, error }` with the message to show, so
   * keep the last good one while the student types:
   *
   *   const f = formula(text);
   *   {f.ok ? <Plot y={(x) => f.fn(x)} /> : <Note tone="danger">{f.error}</Note>}
   */
  export function formula(expression: string, variables?: string[]): Formula;
}

declare module "oculus/3d" {
  import type { ReactNode } from "react";

  /** `[x, y, z]` in the scene's maths coordinates, z up. */
  export type Vec3 = [number, number, number];

  /** A colour of the app's theme, as in `oculus/2d`; a hex or CSS name is an error. */
  export type Colour =
    | "foreground"
    | "muted"
    | "brand"
    | "success"
    | "warning"
    | "danger"
    | "chart-1"
    | "chart-2"
    | "chart-3"
    | "chart-4"
    | "chart-5";

  /**
   * A 3D scene in maths coordinates, z up, the full width of its column and
   * `height` CSS px tall (default 320). The camera starts at `camera`
   * (default `[3.4, 2.6, 2.2]`, above the first octant) looking at `target`
   * (default the origin); dragging orbits about `target` unless `orbit` is
   * false. `zoom` lets the wheel and pinch zoom (off by default: a wheel
   * over the scene scrolls the chat), `pan` lets a right-drag or Shift-drag
   * move the view, `autoRotate` turns it slowly. The background is
   * transparent and the lights are the theme's. Everything else in this module draws inside it, beside any r3f/three
   * elements. It draws only while the widget is in view; out of view it
   * shows a still of its last frame. The view the student orbited to is
   * kept, and state inside the scene starts over when it draws again, so
   * keep it in `useWidgetState` or above the scene.
   *
   * `weight` below multiplies a part's default thickness (a line is about
   * 2 CSS px across at the starting view); thicknesses scale with the
   * starting camera's distance, so a scene framed from further away draws
   * them as thick on screen.
   */
  export function Scene3D(props: {
    children?: ReactNode;
    height?: number;
    camera?: Vec3;
    target?: Vec3;
    orbit?: boolean;
    zoom?: boolean;
    pan?: boolean;
    autoRotate?: boolean;
  }): ReactNode;

  /**
   * The x, y and z axes through the origin, each from `-size` to `size`
   * (one half-length, or `[x, y, z]`; default 1.5) with an arrow at the
   * positive end and its name at the tip unless `labels` is false (default
   * colour `muted`).
   */
  export function Axes3D(props: { size?: number | Vec3; labels?: boolean; color?: Colour }): ReactNode;

  /**
   * A lit sphere of `radius` (default 1) about `center` (default colour
   * `brand`). Below full `opacity` it is see-through, so what is inside
   * shows. `wire` draws its circles of latitude and longitude instead (z
   * is the polar axis); for both, draw two spheres.
   */
  export function Sphere(props: {
    center?: Vec3;
    radius?: number;
    color?: Colour;
    opacity?: number;
    wire?: boolean;
  }): ReactNode;

  /** An arrow from `tail` (default the origin) to `tip` (default colour
   *  `foreground`); its head stops growing on a long arrow. Zero length draws nothing. */
  export function Arrow3D(props: { tail?: Vec3; tip: Vec3; color?: Colour; weight?: number }): ReactNode;

  /**
   * The shorter arc around `center` (default the origin) from direction
   * `from` to direction `to`, both vectors from the centre, `radius` from it
   * (default `|from|`): the angle between two vectors. Opposite directions
   * turn about z (about x when `from` is vertical); the same direction draws
   * nothing.
   */
  export function Arc3D(props: {
    center?: Vec3;
    from: Vec3;
    to: Vec3;
    radius?: number;
    color?: Colour;
    weight?: number;
  }): ReactNode;

  /**
   * A space curve `xyz={(t) => [x, y, z]}` over `domain`, sampled `samples`
   * times (default 200; default colour `chart-1`). Points that aren't finite
   * break the curve: the pieces either side are not joined.
   */
  export function Curve3D(props: {
    xyz: (t: number) => Vec3;
    domain: [min: number, max: number];
    samples?: number;
    color?: Colour;
    weight?: number;
  }): ReactNode;

  /**
   * A surface, lit on both sides (default colour `chart-1`): the graph
   * `z={(x, y) => …}` over `x` × `y`, or the parametric
   * `xyz={(u, v) => [x, y, z]}` over `u` × `v`, on a `samples` × `samples`
   * grid (default 48). `wire` draws its grid lines instead; below full
   * `opacity` it is see-through. A sample that isn't finite leaves a hole.
   */
  export function Surface(
    props: { samples?: number; color?: Colour; opacity?: number; wire?: boolean } & (
      | { z: (x: number, y: number) => number; x: [min: number, max: number]; y: [min: number, max: number] }
      | { xyz: (u: number, v: number) => Vec3; u: [min: number, max: number]; v: [min: number, max: number] }
    ),
  ): ReactNode;

  /**
   * Maths at a point, as TeX: `<Label3D at={[0, 0, 1]} tex={"|0\\rangle"} />` or
   * `<Label3D at={p}>{"\\psi"}</Label3D>`. Drawn flat over the scene, so it
   * always faces the student; centred on `at`, or beside it on the side
   * `attach` names (`"ne"` is above and to the right on screen); hidden
   * while `at` is behind the camera. Inside a `Scene3D` or a `Canvas` only.
   */
  export function Label3D(props: {
    at: Vec3;
    tex?: string;
    children?: string | number;
    attach?: "n" | "ne" | "e" | "se" | "s" | "sw" | "w" | "nw";
    color?: Colour;
  }): ReactNode;
}

declare module "oculus/ask" {
  type JsonType = "object" | "array" | "string" | "number" | "integer" | "boolean" | "null";

  /**
   * The JSON Schema keywords `ask` takes; any other is refused before a model
   * is asked. `additionalProperties` is a boolean (default true), `items` one
   * schema, at most 8 levels deep. A keyword about one type (`minLength`,
   * `properties`, …) applies only to a value of that type.
   */
  export interface JsonSchema {
    type?: JsonType | JsonType[];
    properties?: Record<string, JsonSchema>;
    required?: string[];
    additionalProperties?: boolean;
    items?: JsonSchema;
    enum?: unknown[];
    const?: unknown;
    minItems?: number;
    maxItems?: number;
    minimum?: number;
    maximum?: number;
    minLength?: number;
    maxLength?: number;
    description?: string;
    title?: string;
  }

  /**
   * One model call, answering a JSON value that matches `schema` (checked;
   * a reply that doesn't match is asked again once). It runs on the model the
   * student set for "Widget questions" in Settings → Jobs and is billed to
   * them, so call it on a click, never on mount or per keystroke.
   *
   * The first `ask` or `send` waits until the student allows this widget to
   * use a model. One allowance covers 20 requests (`ask` and `send`
   * together); one request runs at a time. The prompt is at most 20,000
   * characters, the schema 8,000 as JSON. It rejects with an `Error` whose
   * message says why, among them:
   * - "The student didn't allow this widget to use a model."
   * - "This widget has used its 20 requests."
   * - "A request is already running."
   * - "No model is set for Widget questions — choose one in Settings → Jobs."
   * - "The request was stopped." (the student pressed Stop)
   * - "The model's reply did not match the schema: $.steps[2]: expected string"
   * - `schema $.properties.x: "oneOf" is not supported`
   */
  export function ask<T = unknown>(prompt: string, schema: JsonSchema): Promise<T>;

  /**
   * `text` (at most 4,000 characters) as the student's next message in the
   * chat, as if they typed it: the agent answers in the thread. Allowed and
   * counted like `ask`; it also rejects while a reply is running or outside
   * a conversation.
   */
  export function send(text: string): Promise<void>;

  /**
   * `ask` and `send` for a component, with `busy` while one runs and `error`
   * the last failure's message (cleared by the next call). These never
   * reject: a failure resolves `undefined` and sets `error`.
   */
  export function useAsk(): {
    ask: <T = unknown>(prompt: string, schema: JsonSchema) => Promise<T | undefined>;
    send: (text: string) => Promise<void | undefined>;
    busy: boolean;
    error: string | null;
  };
}

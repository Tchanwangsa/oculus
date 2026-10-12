// The globals every Studio component sees: the prelude (studio-core's
// `src/host/prelude/*.js`), typed. A component file imports nothing; these
// are simply there. `src/host/tests/globals.rs` checks that the `declare`d
// names below are exactly the prelude's globals.
//
// Box, Text, Image, Path and Morph are host element tags (plain strings at
// run time); they are typed as components so JSX checks their props. Math is
// JS's own Math object, which the JSX factory knows as the maths element; a
// call signature merged into its interface types `<Math>`.

/** The scene's theme: tokens only. Section names are fixed; token names are the theme's own (these are the bundled themes'). */
interface Theme {
  color: {
    bg: string;
    surface: string;
    surfaceAlt: string;
    text: string;
    muted: string;
    faint: string;
    border: string;
    accent: string;
    accentStrong: string;
    accentSoft: string;
    onAccent: string;
    ink: string;
    onInk: string;
    shadow: string;
    positive: string;
    warning: string;
    negative: string;
    [name: string]: string;
  };
  /** `mono` is for code only; numbers take `fontVariantNumeric: "tabular-nums"`. */
  font: { heading: string; body: string; mono: string };
  /** Style fragments to spread into `style`: `{ ...theme.type.h1, color: theme.color.text }`. */
  type: {
    display: TypeToken;
    h1: TypeToken;
    h2: TypeToken;
    h3: TypeToken;
    body: TypeToken;
    label: TypeToken;
    caption: TypeToken;
    /** Monospace, for code. */
    code: TypeToken;
    [name: string]: TypeToken;
  };
  space: { xxs: number; xs: number; s: number; m: number; l: number; xl: number; xxl: number; [name: string]: number };
  radius: { s: number; m: number; l: number; xl: number; pill: number; [name: string]: number };
  motion: {
    duration: { fast: number; base: number; slow: number; stagger: number; [name: string]: number };
    /** Cubic-bezier control points; pass a name (`"enter"`) as an `easing` prop. */
    easing: { standard: BezierPoints; enter: BezierPoints; exit: BezierPoints; [name: string]: BezierPoints };
    spring: { gentle: SpringOptions; snappy: SpringOptions; bouncy: SpringOptions; [name: string]: SpringOptions };
    transition?: unknown;
    magicMove?: string;
    morphColor?: string;
    motionBlur?: string;
  };
  /** Seconds of silence before and after a scene's narration clip. */
  narration: { lead: number; tail: number };
  /** How burned-in captions look when video.json sets `captions: true`. They sit on a plate
   *  `margin` (a fraction of the height) from the bottom or top edge; keep key content clear of it.
   *  Colours are `color` token names or #hex; sizes are px on a 1080 px short side. */
  captions: {
    fontFamily: string;
    fontSize: number;
    fontWeight: number;
    color: string;
    /** Opacity of the words not yet said. */
    upcoming: number;
    highlight: string;
    highlightText: string;
    highlightStyle: 'color' | 'pill';
    /** A colour, or `"none"`. */
    plate: string;
    plateOpacity: number;
    position: 'bottom' | 'top';
    margin: number;
    maxChars: number;
    maxLines: number;
  };
}

type TypeToken = Pick<
  Style,
  'fontFamily' | 'fontSize' | 'fontWeight' | 'lineHeight' | 'letterSpacing' | 'fontVariantNumeric'
>;

/** A component's second argument. Frozen. */
interface Ctx {
  /** Seconds since the scene (or the enclosing Sequence) started. */
  t: number;
  /** The scene's (or the enclosing Sequence's) length in seconds. */
  duration: number;
  theme: Theme;
  size: { width: number; height: number };
}

/** What JSX makes. Opaque: return it, nest it, never inspect it. */
interface StudioElement {
  readonly __element: true;
}

type Child = StudioElement | string | number | boolean | null | undefined | Child[];
type Children = Child | Child[];

/** px, `"N%"` of the parent, or `"auto"`. */
type Dim = number | `${number}%` | 'auto';
/** px: all sides, `[vertical, horizontal]` or `[top, right, bottom, left]`. */
type Sides = number | [number] | [number, number] | [number, number, number, number];
/** `#rgb`, `#rrggbb` or `#rrggbbaa`. Read colours from `theme.color`; lint refuses literals. */
type Color = string;
/** `{ linear: { angle, stops } }`: a CSS angle in degrees (0 is to top, 90 to right), offsets rising 0 to 1. */
type Gradient = { linear: { angle: number; stops: [number, Color][] } };
type Fill = Color | Gradient;
type Align = 'start' | 'end' | 'flex-start' | 'flex-end' | 'center' | 'stretch' | 'baseline';
type Justify =
  | 'start'
  | 'end'
  | 'flex-start'
  | 'flex-end'
  | 'center'
  | 'space-between'
  | 'space-around'
  | 'space-evenly';
type OriginKeyword = 'center' | 'top' | 'bottom' | 'left' | 'right';
/** `[fx, fy]` fractions of the border box, or keywords such as `"center"` or `"top left"`. */
type Origin = [number, number] | OriginKeyword | `${OriginKeyword} ${OriginKeyword}`;

/** Show a text unit by unit as `progress` goes 0 to 1 (paint only: layout is the whole text's). */
interface Reveal {
  progress: number;
  /** Default `"word"`. */
  by?: 'char' | 'word' | 'line';
  /** 0 is all together, 1 strictly one after another. Default 0.5. */
  stagger?: number;
  /** Default `"fade"`. */
  from?: 'fade' | 'rise' | 'blur';
}

/** The closed set of style keys; an unknown key is an error naming the closest. */
interface Style {
  width?: Dim;
  height?: Dim;
  minWidth?: Dim;
  minHeight?: Dim;
  maxWidth?: Dim;
  maxHeight?: Dim;
  /** Default `"row"`, as in CSS. Use Stack for a column. */
  flexDirection?: 'row' | 'column' | 'row-reverse' | 'column-reverse';
  flexWrap?: 'nowrap' | 'wrap';
  flexGrow?: number;
  flexShrink?: number;
  gap?: number;
  padding?: Sides;
  margin?: Sides;
  alignItems?: Align;
  alignSelf?: Align;
  justifyContent?: Justify;
  position?: 'relative' | 'absolute';
  left?: Dim;
  top?: Dim;
  right?: Dim;
  bottom?: Dim;
  fill?: Fill;
  radius?: number;
  border?: { width: number; color: Color };
  /** `blur` is box-shadow's blur radius. */
  shadow?: { x?: number; y?: number; blur?: number; spread?: number; color: Color };
  /** filter: blur(), in px of gaussian sigma. Composites the node and its children as one group. */
  blur?: number;
  /** 0 to 1. Composites the node and its children as one group. */
  opacity?: number;
  /** Clip children to this node's rounded rect. */
  clip?: boolean;
  translateX?: number;
  translateY?: number;
  scale?: number;
  /** Degrees. */
  rotate?: number;
  /** Default the centre. */
  transformOrigin?: Origin;
  /** `[top, right, bottom, left]` fractions of the border box; clips the node's whole paint. */
  clipInset?: [number, number, number, number];
  /** A bundled family (Inter, Manrope, JetBrains Mono). Read it from `theme.font` or a `theme.type` token; lint refuses literals. */
  fontFamily?: string;
  fontSize?: number;
  fontWeight?: number;
  color?: Color;
  /** A multiple of the font size. */
  lineHeight?: number;
  letterSpacing?: number;
  textAlign?: 'left' | 'center' | 'right' | 'justify';
  fontVariantNumeric?: 'normal' | 'tabular-nums';
  reveal?: Reveal;
  /** Image only. */
  objectFit?: 'fill' | 'cover' | 'contain';
}

interface Stroke {
  /** In viewBox units. */
  width: number;
  color: Color;
  /** Default `"butt"`. */
  cap?: 'butt' | 'round' | 'square';
  /** Default `"miter"`. */
  join?: 'miter' | 'round' | 'bevel';
  /** On and off lengths, as SVG stroke-dasharray. */
  dash?: number[];
  dashOffset?: number;
}

type BezierPoints = [number, number, number, number];
type EasingFn = (p: number) => number;
/** An easing function, cubic-bezier points, or a `theme.motion.easing` name (`"enter"`, `"exit"`, `"standard"`). */
type Easing = EasingFn | BezierPoints | string;
interface SpringOptions {
  stiffness?: number;
  damping?: number;
  mass?: number;
}

/** A props schema entry. Chain `.default(v)` or `.optional()`; with neither the prop is required. */
interface PropBuilder<T> {
  default(value: T): PropBuilder<T>;
  optional(): PropBuilder<T | undefined>;
}

/** Any function component: `(props, ctx) => element`. */
type Component<P> = (props: P, ctx: Ctx) => StudioElement;

interface BoxProps {
  style?: Style;
  children?: Children;
}
interface FlexProps extends BoxProps {
  gap?: number;
  /** alignItems. */
  align?: Align;
  /** justifyContent. */
  justify?: Justify;
  /** flexWrap: wrap. */
  wrap?: boolean;
}

/** A box: layout (flexbox), fill, border, shadow, transforms. Children are elements. */
declare const Box: Component<BoxProps>;
/** Text. Children are strings or numbers only; to mix styles put several Text in a Row. */
declare const Text: Component<{ style?: Style; children?: string | number | (string | number)[] }>;
/** An image from the video's `assets/`. A missing file is an error. */
declare const Image: Component<{ src: string; style?: Style }>;
/**
 * SVG path data, fitted into its box (`xMidYMid meet`) from `viewBox` (default: the path's bounds).
 * `trim: [start, end]` strokes only that fraction of the length.
 */
declare const Path: Component<{
  d: string;
  viewBox?: [number, number, number, number];
  fill?: Fill;
  stroke?: Stroke;
  trim?: [number, number];
  style?: Style;
}>;
/** How a Morph (or Steps) spends its `p`: stagger, fades and each piece's easing. */
interface MorphTiming {
  /**
   * 0 to 1, manim's lag_ratio: pieces start in reading order (A's leaves, then B's arrivals), each `lag` of a piece's
   * run after the last. Default 0, all together. With lag, pass a linear `p` and an `ease`.
   */
  lag?: number;
  /**
   * `"overlap"` (default): unmatched pieces fade out over 0 to 0.6 of their run and in over 0.4 to 1, each riding
   * beside its nearest matched neighbour, as TransformMatchingTex does. `"sequential"`: out over 0 to 0.4, in over
   * 0.4 to 1, in place.
   */
  fade?: 'overlap' | 'sequential';
  /** Each piece's own easing (sampled): a function, cubic-bezier points or a theme easing name. Default linear. */
  ease?: Easing;
}
/**
 * Two layouts of one place, blended at `p` (0 to 1; an overshooting easing may pass either end, and geometry follows
 * while opacity and colour stop there): FLIP-style layout animation. Exactly two element children; takes no key.
 */
declare const Morph: Component<MorphTiming & { p: number; children: [StudioElement, StudioElement] }>;
/**
 * N states of one place, a derivation rewritten in place: a Morph between the pair the time picks. Before `at[0]` the
 * first state shows; transition k runs from `at[k]` for `dur` (default `theme.motion.duration.base`), cut short by
 * the next cue, so each cue is continuous. `ease` defaults to easeInOutCubic. A key in states 0 and 2 but not 1
 * fades out over the first transition and back in, at its state-2 place, over the second. States may be render
 * props, `(ctx) => element`.
 */
declare const Steps: Component<
  MorphTiming & {
    at: number[];
    dur?: number;
    children: (StudioElement | ((ctx: Ctx) => StudioElement))[];
  }
>;
/**
 * Writes every `<Math>` inside it on, manim's Write: each piece's outline is stroked on, then its fill fades in, the
 * pieces of a group one after another in reading order. `at` is seconds (the whole formula) or `{ group: seconds }`
 * naming `\k` groups, each cued on its own word; a piece in no listed group writes with the nearest listed group
 * before it, else the first. Math only (for words, `\text{…}` inside a Math).
 */
declare const Write: Component<{
  at?: number | Record<string, number>;
  /** Each group's length; default `theme.motion.duration.slow`. */
  dur?: number | Record<string, number>;
  /** 0 to 1 within a group; default manim's `min(4 / (n + 1), 0.2)`. */
  lag?: number;
  /** Outline width in em. Default 0.025. */
  stroke?: number;
  children?: Children | ((ctx: Ctx) => StudioElement);
}>;
/**
 * When an overlay draws on: `progress` 0 to 1, or from `at` for `dur` (default `theme.motion.duration.base`) eased
 * by `ease` (default easeInOutCubic); with neither it is drawn.
 */
interface OverlayTiming {
  at?: number;
  dur?: number;
  ease?: Easing;
  progress?: number;
  style?: Style;
}
/**
 * A key or a key prefix: `"eq/lhs"` covers `eq/lhs`, `eq/lhs.1`, … and `eq/lhs/…`. Its box is the union of the
 * nodes it covers after layout and after any morph, so an overlay follows its target while it moves (a fading term
 * counts by its opacity, and an overlay on terms that have faded out fades too).
 */
type Target = string;
/**
 * KaTeX's brace along one side of a target, `label` (a string or an element) beyond it; grows from its centre, then
 * the label fades in. Size and gaps default to the target's maths size.
 */
declare const Brace: Component<
  OverlayTiming & {
    target: Target;
    /** Default `"below"`. */
    side?: 'below' | 'above' | 'left' | 'right';
    /** A string is set at 0.7 of the target's em; an element keeps its own size. */
    label?: string | StudioElement;
    /** px between the target and the brace; default 0.12 em. */
    gap?: number;
    /** px per em of the brace (its thickness); default the target's. */
    size?: number;
    /** Default `theme.color.text`. */
    color?: Color;
  }
>;
/** A rounded rectangle round a target, drawn on clockwise from its top left. */
declare const Surround: Component<
  OverlayTiming & {
    target: Target;
    /** px; defaults 0.2 em, corners 0.12 em, stroke 0.045 em. */
    pad?: number;
    radius?: number;
    width?: number;
    /** Default `theme.color.accent`. */
    color?: Color;
  }
>;
/** A line under a target, drawn on from the left. */
declare const Underline: Component<
  OverlayTiming & {
    target: Target;
    /** px below the target; default 0.1 em. */
    gap?: number;
    width?: number;
    /** Default `theme.color.accent`. */
    color?: Color;
  }
>;
/** The target's own terms pulse: scale to `scale` (default 1.2) about their joint centre and flash to `color` (default the accent), then back. */
declare const Indicate: Component<{
  target: Target;
  scale?: number;
  color?: Color;
  /** One of `at` or `progress` is required. `dur` defaults to `theme.motion.duration.slow`. */
  at?: number;
  dur?: number;
  progress?: number;
}>;
/**
 * `<Math tex="…">`: LaTeX typeset by KaTeX as one glyph per term. Its box is the formula's size and its
 * baseline the formula's, so it sits in a Row with `align: "baseline"`. `style` takes `fontSize` (the size of the
 * text it sits with; maths is set 1.21 times larger, as KaTeX does) and `color`, plus a Box's placement and paint
 * keys; other text keys are ignored, so `{ ...theme.type.body }` works, and keys that size, pad or lay out
 * children are refused. `\color{accent}` takes a theme colour name or hex. In a JSX attribute
 * (`tex="\frac{a}{b}"`) a backslash is itself; in a JS string (`tex={"\\frac{a}{b}"}`) write it twice.
 *
 * Every glyph, rule and root sign is keyed `<key>/<term>` for magic move: by character, `^` or `_` per script
 * it sits in (exponents, limits above and root indices are `^`; subscripts and limits below `_`), then
 * occurrence (`eq/x`, `eq/2^`, `eq/x#2`), so an exponent never matches a baseline digit; or `\k{name}{…}`
 * names a group (`eq/name`, `eq/name.1`, … in reading order), which overrides. Two
 * Math elements with one key in a `<Morph>` glide the terms they share and fade the rest. Without a key, a
 * Math inside a Morph is keyed by its place there; elsewhere its terms have no keys. A key shared by consecutive
 * scenes makes their transition a magic move, so a formula carries across scenes term by term.
 */
interface Math {
  (props: {
    tex: string;
    /** Display style (limits above and below, big operators), the default; `false` for inline maths. */
    display?: boolean;
    style?: Style;
  }): StudioElement;
}
/** A Box with `flexDirection: "column"`. */
declare const Stack: Component<FlexProps>;
/** A Box with `flexDirection: "row"`. */
declare const Row: Component<FlexProps>;
/** `<>...</>`; its children count one by one in a Stagger. */
declare const Fragment: Component<{ children?: Children }>;

/**
 * Gives its subtree `t - from`, clamped to `[0, dur]`, and `duration = dur` (default: the rest of the parent's).
 * Outside `[from, from + dur)` the subtree keeps its layout slot but is hidden; `hold` keeps it shown after.
 * Children may be render props, `(ctx) => element`.
 */
declare const Sequence: Component<{
  from?: number;
  dur?: number;
  hold?: boolean;
  children?: Children | ((ctx: Ctx) => StudioElement) | ((ctx: Ctx) => StudioElement)[];
}>;
/** Child i starts at `at + i * each` seconds, as if in a Sequence. */
declare const Stagger: Component<{ each: number; at?: number; children?: Children }>;

type Preset = 'fade' | 'rise' | 'slide-left' | 'slide-right' | 'scale' | 'blur' | 'mask';
interface PresetProps {
  /** Default `"fade"`. */
  preset?: Preset;
  /** Seconds into the current ctx. Enter defaults to 0; Exit to `duration - dur`. */
  at?: number;
  /** Default `theme.motion.duration.base`. */
  dur?: number;
  /** Default `theme.motion.easing.enter` (Enter) or `.exit` (Exit). */
  easing?: Easing;
  children?: Children;
}
/** Animates its children in. Composes onto them (opacity and scale multiply, translate and blur add); does not shift time. */
declare const Enter: Component<PresetProps>;
/** Animates its children out, carrying on in the entry's direction. */
declare const Exit: Component<PresetProps>;
/** A full-frame Box whose transform puts frame point (x, y) (default the centre) at the frame centre, zoomed and rotated about it. */
declare const Camera: Component<{
  x?: number;
  y?: number;
  zoom?: number;
  /** Degrees. */
  rotate?: number;
  style?: Style;
  children?: Children;
}>;
/**
 * A stroke through `points` (frame px) drawn up to `progress` (0 to 1), with a head riding the drawn end along the
 * shaft's tangent there (it grows in over the first head length of shaft), so the arrow reads as being drawn.
 * Two absolute full-frame Paths: its parent must fill the frame.
 *
 * With `from` and `to` instead, each a Target or a point in its parent's px, it is an overlay: a keyed end starts
 * `gap` (default 0.1 em) off its target's box on the line between the ends, and it follows the target as it moves.
 */
declare const Arrow: Component<
  | {
      points: [number, number][];
      /** 0 is straight segments; otherwise a smooth curve. Default 0. */
      curve?: number;
      /** Default 1. */
      progress?: number;
      /** Head size in px; 0 for none. Default 4.5 x the stroke width. */
      head?: number;
      /** Defaults: width 4, colour `theme.color.text`, round cap and join. */
      stroke?: Partial<Stroke>;
      style?: Style;
    }
  | (OverlayTiming & {
      from: Target | [number, number];
      to: Target | [number, number];
      /** Degrees the shaft turns through, an arc; 0 is straight. */
      bend?: number;
      head?: number;
      gap?: number;
      /** `width` and `color` only. */
      stroke?: Partial<Stroke>;
    })
>;

/** A string prop. */
declare function str(): PropBuilder<string>;
/** A number prop. */
declare function num(): PropBuilder<number>;
/** A boolean prop. */
declare function bool(): PropBuilder<boolean>;
/** A string of LaTeX source. */
declare function latex(): PropBuilder<string>;
/** A list of one builder's values: `list(str())`. */
declare function list<T>(of: PropBuilder<T>): PropBuilder<T[]>;
/** One of the given values: `oneOf(["left", "right"])`. */
declare function oneOf<T extends string | number>(values: T[]): PropBuilder<T>;
/**
 * A theme token, given by name and handed to the component as its value:
 * `token("color").default("accent")` takes `"positive"` and gives `theme.color.positive`.
 * Any section works, dotted ones too (`token("motion.spring")`).
 */
declare function token(section: string): PropBuilder<any>;

declare function clamp(x: number, lo: number, hi: number): number;
declare function lerp(a: number, b: number, p: number): number;
declare function linear(p: number): number;
declare function easeOutCubic(p: number): number;
declare function easeInOutCubic(p: number): number;
/** CSS cubic-bezier(x1, y1, x2, y2) as an easing function. */
declare function cubicBezier(x1: number, y1: number, x2: number, y2: number): EasingFn;
/**
 * Maps `t` through rising `inputs` to `outputs`, piecewise linear, clamped to the ends unless `clamp: false`.
 * `easing` must be a function here (`easeOutCubic`, `cubicBezier(...theme.motion.easing.enter)`).
 */
declare function interpolate(
  t: number,
  inputs: number[],
  outputs: number[],
  options?: { easing?: EasingFn; clamp?: boolean },
): number;
/** A damped spring from 0 to 1, closed form, so any `t` samples directly. Defaults 170, 26, 1. */
declare function spring(t: number, options?: SpringOptions): number;
/** Blends two hex colours: `mix(theme.color.surface, theme.color.accent, p)`. */
declare function mix(a: Color, b: Color, p: number): Color;
/** Progress of `[at, at + dur]` at `t`, 0 to 1, for `reveal.progress`. Linear unless `easing` is given. */
declare function revealProgress(t: number, at: number, dur: number, easing?: Easing): number;
/** A number counting from `from` to `to`, as a fixed-decimals string. Defaults: at 0, dur 1, easeOutCubic, 0 decimals. */
declare function counter(
  t: number,
  from: number,
  to: number,
  options?: { at?: number; dur?: number; easing?: Easing; decimals?: number },
): string;
/** Start times for durations played one after another, `gap` apart, from `at`. */
declare function series(durations: number[], options?: { gap?: number; at?: number }): number[];
/** Start times for durations that all start at `at`. */
declare function parallel(durations: number[], options?: { at?: number }): number[];

/**
 * When the nth occurrence of `phrase` starts in this scene's narration, in scene seconds (lead included).
 * Throws in a silent scene and in `gallery`, and at a phrase the narration does not say (naming the closest).
 */
declare function word(phrase: string, nth?: number): number;
/** When the nth occurrence of `phrase` ends (its last word's end), in scene seconds. */
declare function wordEnd(phrase: string, nth?: number): number;
/**
 * When the nth `[pause]` in this scene's script.md starts, in scene seconds: the narration is silent from here
 * for the pause's length. Land a beat just before it and hold it. Throws like `word()`, and past the last pause.
 */
declare function pause(nth?: number): number;

/** Goes to the frame's logs (`oculus studio frame` prints them), at most 200 lines a call. */
declare const console: {
  log(...args: unknown[]): void;
  info(...args: unknown[]): void;
  warn(...args: unknown[]): void;
  error(...args: unknown[]): void;
};

declare namespace JSX {
  type Element = StudioElement;
  interface IntrinsicElements {}
  interface IntrinsicAttributes {
    /** Unique within the scene. A key shared by two consecutive scenes makes their transition a magic move. */
    key?: string | number;
  }
  interface ElementChildrenAttribute {
    children: {};
  }
}

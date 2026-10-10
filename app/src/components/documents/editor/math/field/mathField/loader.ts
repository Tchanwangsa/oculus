import { StateEffect } from "@codemirror/state";
import { ViewPlugin, type EditorView } from "@codemirror/view";

import { modelOf } from "./model";

export type MathLive = typeof import("mathlive");

export type LoadState = "loading" | "ready" | "failed";

/** The loaded MathLive module, or null until it has arrived (a live binding). */
export let lib: MathLive | null = null;
/** Where the import stands (a live binding). */
export let loadState: LoadState = "loading";
let loading: Promise<void> | null = null;
const waiting = new Set<EditorView>();
export const mathLiveSettled = StateEffect.define<LoadState>();

export function loadMathLive(): Promise<void> {
  loading ??= Promise.all([import("mathlive"), import("mathlive/static.css?raw")]).then(
    ([m, css]) => {
      configure(m);
      staticStyles(css.default);
      lib = m;
      settle("ready");
    },
    () => settle("failed"),
  );
  return loading;
}

/** MathLive's stylesheet for static markup (`staticMath`), once, without
 *  its `@font-face` rules: KaTeX's CSS declares the same families. */
function staticStyles(css: string) {
  const style = document.createElement("style");
  style.dataset.mathliveStatic = "";
  style.textContent = css.replace(/@font-face\s*\{[^}]*\}/g, "");
  document.head.append(style);
}

function settle(state: LoadState) {
  loadState = state;
  for (const view of waiting) view.dispatch({ effects: mathLiveSettled.of(state) });
  waiting.clear();
}

/** MathLive has loaded: rendered maths is drawn with it (`staticMath`). */
export function mathLiveReady(): boolean {
  return lib != null;
}

/** Statics, once: our bundle already carries KaTeX's fonts (MathLive reuses
 *  them when every family is in `document.fonts`), and nothing may reach
 *  the network or make a sound. */
function configure(m: MathLive) {
  const MF = m.MathfieldElement;
  MF.fontsDirectory = null;
  MF.soundsDirectory = null;
  MF.keypressSound = null;
  MF.plonkSound = null;
  MF.computeEngine = null;
  patchArrays(MF);
}

/** Row stretch for arrays and matrices, in KaTeX (`live-preview/widgets/math.ts`) and here. */
export const MATH_ARRAYSTRETCH = 1.2;
/** Space between a display block's top-level `\\` lines, in em: KaTeX's
 *  `.katex-newline` (`theme/math.ts`) and the field's root `lines` table. */
export const MATH_LINE_GAP = 0.5;

/**
 * Layout fixes on MathLive's internal array atom (recheck on upgrade), so
 * entering a block doesn't move it: `array` sits centred on the maths axis
 * as in LaTeX and KaTeX (MathLive hangs it from its first row, which pads a
 * `\left[ \begin{array}…\right]` above), drops its outer column padding
 * when it is all a `\left…\right` holds, as KaTeX's rendering does
 * (`hugArrays` in `live-preview/widgets/math.ts`), and the root `lines` table
 * (`\displaylines`) takes `MATH_LINE_GAP` between rows while other arrays
 * take `MATH_ARRAYSTRETCH`. The class is reached through a throwaway field.
 */
function patchArrays(MF: MathLive["MathfieldElement"]) {
  const probe = new MF();
  probe.value = "\\begin{array}{c}x\\end{array}";
  probe.style.cssText = "position: fixed; left: -9999px; visibility: hidden";
  document.body.append(probe);
  const array = modelOf(probe)?.at(1)?.parent;
  probe.remove();
  if (array?.type !== "array") return;
  type ArrayAtom = {
    environmentName: string;
    arraystretch?: number;
    leftDelim?: string;
    rightDelim?: string;
    parent?: { type: string; body?: { type: string }[] };
  };
  type Context = { getRegisterAsEm(name: string, precision?: number): number };
  const proto = Object.getPrototypeOf(array) as { render(this: ArrayAtom, context: object): unknown };
  const render = proto.render;
  const spaced = new WeakSet<object>();
  let contextPatched = false;
  proto.render = function (context) {
    if (this.environmentName === "array") {
      const { leftDelim, rightDelim, parent } = this;
      // Delimiters of "." draw none and add no outer padding.
      const hug = parent?.type === "leftright" && parent.body?.filter((a) => a.type !== "first").length === 1;
      this.environmentName = "matrix";
      if (hug) this.leftDelim = this.rightDelim = ".";
      try {
        return stretched(this, MATH_ARRAYSTRETCH, () => render.call(this, context));
      } finally {
        this.environmentName = "array";
        if (hug) Object.assign(this, { leftDelim, rightDelim });
      }
    }
    if (this.environmentName !== "lines") return stretched(this, MATH_ARRAYSTRETCH, () => render.call(this, context));
    // The table reads `jot` from a context whose parent is this one.
    if (!contextPatched) {
      contextPatched = true;
      const cx = Object.getPrototypeOf(context) as Context & { parent?: object };
      const em = cx.getRegisterAsEm;
      cx.getRegisterAsEm = function (this: Context & { parent?: object }, name, precision) {
        return name === "jot" && this.parent && spaced.has(this.parent) ? MATH_LINE_GAP : em.call(this, name, precision);
      };
    }
    spaced.add(context);
    try {
      return render.call(this, context);
    } finally {
      spaced.delete(context);
    }
  };
}

/** Render with `stretch` unless the environment sets its own (`cases`,
 *  `smallmatrix`), as KaTeX's `\arraystretch` macro applies. */
function stretched<T>(atom: { arraystretch?: number }, stretch: number, render: () => T): T {
  if (atom.arraystretch !== undefined) return render();
  atom.arraystretch = stretch;
  try {
    return render();
  } finally {
    delete atom.arraystretch;
  }
}

/** Starts the import as a Live editor mounts, so the first click into maths
 *  finds MathLive ready. */
export const loader = ViewPlugin.fromClass(
  class {
    constructor(readonly view: EditorView) {
      if (loadState !== "loading") return;
      waiting.add(view);
      void loadMathLive();
    }
    destroy() {
      waiting.delete(this.view);
    }
  },
);

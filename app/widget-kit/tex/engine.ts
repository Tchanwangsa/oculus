/**
 * The maths engine inside a widget frame: the app's `math-core` (KaTeX's
 * output, docs/editor-maths.md) and KaTeX's stylesheet and fonts, all served
 * from the kit. A chunk of its own, loaded the first time a `Tex` mounts.
 * The wasm is fetched and compiled here (CSP `connect-src` and
 * `'wasm-unsafe-eval'`); the stylesheet sits beside `kit.css`.
 */
import { initSync, renderToString } from "../../math-core/pkg/oculus_math.js";

const WASM = new URL("../../math-core/pkg/oculus_math_bg.wasm", import.meta.url);

async function engine() {
  const res = await fetch(WASM);
  if (!res.ok) throw new Error(`the maths engine answered ${res.status}`);
  initSync({ module: await WebAssembly.compile(await res.arrayBuffer()) });
}

/** KaTeX's stylesheet, resolved against the `kit.css` link `srcdoc` marks. */
function stylesheet(): Promise<void> {
  const kit = document.querySelector<HTMLLinkElement>("link[data-oculus-kit]");
  if (!kit) return Promise.reject(new Error("the frame has no kit stylesheet"));
  const link = document.createElement("link");
  link.rel = "stylesheet";
  link.href = new URL("katex/katex.min.css", kit.href).href;
  return new Promise((resolve, reject) => {
    link.onload = () => resolve();
    link.onerror = () => reject(new Error("KaTeX's stylesheet did not load"));
    document.head.append(link);
  });
}

/** Resolves once both are in; rejects with what failed. */
export function loadEngine(): Promise<void> {
  return Promise.all([engine(), stylesheet()]).then(() => undefined);
}

/** KaTeX HTML for `tex`; a parse error comes back as KaTeX's red source. */
export function renderTex(tex: string, display: boolean): string {
  return renderToString(tex, { displayMode: display, throwOnError: false });
}

/**
 * What `katex` resolves to in the kit build (`vite.widget-kit.config.ts`):
 * Mafs imports KaTeX at its top level, and the kit typesets with the app's
 * own maths engine instead. `render` is called synchronously (Mafs' `LaTeX`
 * does it in an effect) while the engine may still be loading, so it shows
 * the TeX muted until the engine is in, then draws the last TeX asked of
 * that element if the element is still on the page.
 */
import { loadTex, texEngine } from "../tex/load";

interface Renderer {
  renderTex(tex: string, display: boolean): string;
}

/** The parts of an element `render` writes. */
export interface Target {
  innerHTML: string;
  textContent: string | null;
  className: string;
  readonly isConnected: boolean;
}

const escape = (s: string) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");

export function katexShim(load: () => Promise<void>, engine: () => Renderer | null) {
  const waiting = new WeakMap<Target, { tex: string; display: boolean }>();

  function draw(el: Target, renderer: Renderer, tex: string, display: boolean) {
    el.className = "o-tex";
    el.innerHTML = renderer.renderTex(tex, display);
  }

  function render(tex: string, el: Target, options?: { displayMode?: boolean }) {
    const display = !!options?.displayMode;
    const renderer = engine();
    if (renderer) {
      waiting.delete(el);
      return draw(el, renderer, tex, display);
    }
    const first = !waiting.has(el);
    waiting.set(el, { tex, display });
    el.className = "o-tex o-tex-pending";
    el.textContent = tex;
    if (!first) return;
    void load().then(() => {
      const job = waiting.get(el);
      const ready = engine();
      waiting.delete(el);
      if (job && ready && el.isConnected) draw(el, ready, job.tex, job.display);
    });
  }

  function renderToString(tex: string, options?: { displayMode?: boolean }): string {
    const renderer = engine();
    return renderer ? renderer.renderTex(tex, !!options?.displayMode) : escape(tex);
  }

  return { render, renderToString };
}

const katex = katexShim(loadTex, texEngine);
export default katex;
export const { render, renderToString } = katex;

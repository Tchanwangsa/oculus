import { lib } from "./loader";
import { toField } from "./serialize";
import { readsCleanly } from "./visual-state";

/** MathLive's static markup by display flag and source; a note re-renders often. */
const staticCache = new Map<string, string | null>();

/** MathLive's markup for maths the field could open (`readsCleanly`), else
 *  null. It goes through the patched array atom, as the field does. */
function renderStatic(source: string, display: boolean): string | null {
  if (!lib || !readsCleanly(source, display)) return null;
  const key = `${display ? "D" : "I"}${source}`;
  let html = staticCache.get(key);
  if (html === undefined) {
    try {
      html = lib.convertLatexToMarkup(toField(source, display), { defaultMode: display ? "math" : "inline-math" });
    } catch {
      html = null;
    }
    if (staticCache.size >= 500) staticCache.clear();
    staticCache.set(key, html);
  }
  return html;
}

/** Maths drawn by MathLive without a field, in the box a field opened on it
 *  takes (`.cm-math-ml` in `theme/math.ts`), or null to draw it with KaTeX. */
export function staticMath(source: string, display: boolean): HTMLElement | null {
  const html = renderStatic(source, display);
  if (!html) return null;
  const dom = document.createElement("span");
  dom.className = "cm-math-ml";
  dom.innerHTML = html;
  return dom;
}

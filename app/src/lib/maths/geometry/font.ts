/** Px from an inline box's top to its baseline, by the face (family, style,
 *  weight; KaTeX_Math has only italics) and size: WebKit rounds a face's
 *  ascent at each size, so it is measured, not scaled. Fonts arriving
 *  change it, so a load empties the cache. */
const ascents = new Map<string, number>();
let measurer: HTMLElement | null = null;

export function fontAscent(style: CSSStyleDeclaration): number {
  const { fontFamily, fontSize, fontStyle, fontWeight } = style;
  const key = `${fontSize}|${fontStyle}|${fontWeight}|${fontFamily}`;
  const known = ascents.get(key);
  if (known != null) return known;
  if (!measurer?.isConnected) {
    measurer = document.createElement("div");
    measurer.setAttribute("aria-hidden", "true");
    measurer.style.cssText = "position:absolute;left:0;top:0;visibility:hidden;pointer-events:none;contain:layout style size;width:0;height:0;overflow:hidden;white-space:nowrap";
    document.body.append(measurer);
    document.fonts?.addEventListener("loadingdone", () => ascents.clear());
  }
  // A zero-size inline-block's bottom is the baseline of the line it is on.
  const span = document.createElement("span");
  Object.assign(span.style, { fontFamily, fontSize, fontStyle, fontWeight, lineHeight: "normal" });
  const probe = document.createElement("span");
  probe.style.cssText = "display:inline-block;width:0;height:0;vertical-align:baseline";
  span.append(probe);
  measurer.append(span);
  const ascent = probe.getBoundingClientRect().bottom - span.getBoundingClientRect().top;
  span.remove();
  ascents.set(key, ascent);
  return ascent;
}

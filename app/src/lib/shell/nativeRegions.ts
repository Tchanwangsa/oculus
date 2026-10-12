import { invoke } from "@tauri-apps/api/core";
import { appZoom } from "@/pages/tools/browser/viewport";
import { bottomRadii, portalsOver } from "./holeGeometry";

/**
 * Windows' half of `nativeHoles.ts`. A transparent WebView2 never shows a
 * sibling under it, so each page stays above the app and Rust clips it with a
 * window region (`app/src-tauri/src/shell/native_layer/windows.rs`): the open
 * holes minus the portals over them. A popover then draws over the live page,
 * and a page with no hole over it, or every page while the holes are
 * suspended, is hidden. Sent in logical window points, as `measure()` places
 * the pages.
 *
 * Clicks follow the region natively, so a click on the page beside a popover
 * goes to the page and moves focus out of the app's document. While a portal
 * covers a hole, that window `blur` sends Escape, so Radix layers close as
 * they do on macOS; switching to another app closes them the same way.
 */

interface Hole {
  left: number;
  top: number;
  right: number;
  bottom: number;
  bl: number;
  br: number;
}

interface Cover {
  left: number;
  top: number;
  right: number;
  bottom: number;
  radius: number;
}

let sent = "";
let covered = false;

export function sendRegions(slots: HTMLElement[]): void {
  const z = appZoom();
  const n = (v: number) => Math.round(v * z * 100) / 100;
  const holes: Hole[] = [];
  const pages: DOMRect[] = [];
  for (const slot of slots) {
    const r = slot.getBoundingClientRect();
    if (r.width <= 0 || r.height <= 0) continue;
    const [bl, br] = bottomRadii(slot, r);
    holes.push({
      left: n(r.left),
      top: n(r.top),
      right: n(r.right),
      bottom: n(r.bottom),
      bl: n(bl),
      br: n(br),
    });
    pages.push(r);
  }
  const covers: Cover[] = coversOver(pages).map(({ rect, radius }) => ({
    left: n(rect.left),
    top: n(rect.top),
    right: n(rect.right),
    bottom: n(rect.bottom),
    radius: n(radius),
  }));
  setCovered(covers.length > 0);
  const payload = JSON.stringify([holes, covers]);
  if (payload === sent) return;
  sent = payload;
  invoke("native_set_holes", { holes, covers }).catch(() => {});
}

/** The portals over any page, each once. A wrapper and the content filling
 *  it are one cover with the content's radius; anything inside another
 *  cover adds nothing, and its square corners would clip the page where
 *  the outer one is rounded. */
function coversOver(pages: DOMRect[]): { rect: DOMRect; radius: number }[] {
  const found = new Set<Element>();
  for (const page of pages) for (const el of portalsOver(page)) found.add(el);
  const all = [...found]
    .map((el) => ({ rect: el.getBoundingClientRect(), radius: radiusOf(el) }))
    .sort((a, b) => area(b.rect) - area(a.rect));
  const kept: { rect: DOMRect; radius: number }[] = [];
  for (const c of all) {
    const outer = kept.find((k) => contains(k.rect, c.rect));
    if (!outer) kept.push(c);
    else if (same(outer.rect, c.rect)) outer.radius = Math.max(outer.radius, c.radius);
  }
  return kept;
}

/** The largest corner radius, in CSS px. */
function radiusOf(el: Element): number {
  const cs = getComputedStyle(el);
  const corners = [
    cs.borderTopLeftRadius,
    cs.borderTopRightRadius,
    cs.borderBottomLeftRadius,
    cs.borderBottomRightRadius,
  ];
  return Math.max(...corners.map((v) => parseFloat(v) || 0));
}

function area(r: DOMRect): number {
  return r.width * r.height;
}

function contains(outer: DOMRect, inner: DOMRect): boolean {
  return (
    inner.left >= outer.left - 0.5 &&
    inner.top >= outer.top - 0.5 &&
    inner.right <= outer.right + 0.5 &&
    inner.bottom <= outer.bottom + 0.5
  );
}

function same(a: DOMRect, b: DOMRect): boolean {
  return contains(a, b) && contains(b, a);
}

function setCovered(next: boolean): void {
  if (next === covered) return;
  covered = next;
  if (next) window.addEventListener("blur", dismiss);
  else window.removeEventListener("blur", dismiss);
}

/** Radix closes its topmost layer on an Escape keydown at the document. */
function dismiss(): void {
  document.dispatchEvent(
    new KeyboardEvent("keydown", { key: "Escape", code: "Escape", bubbles: true, cancelable: true }),
  );
}

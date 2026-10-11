import type { Viewport } from "@/lib/browser";

export function appZoom(): number {
  const z = parseFloat(
    document.documentElement.style.getPropertyValue("--app-zoom"),
  );
  return Number.isFinite(z) && z > 0 ? z : 1;
}

/** The slot as window insets in logical points (CSS px × page zoom), plus the
 *  card's inner radius for the page's bottom corners. */
export function measure(slot: HTMLElement): Viewport {
  const z = appZoom();
  const r = slot.getBoundingClientRect();
  let radius = 0;
  const card = slot.closest("main");
  if (card) {
    const cs = getComputedStyle(card);
    radius = Math.max(
      0,
      parseFloat(cs.borderBottomLeftRadius) - parseFloat(cs.borderLeftWidth),
    );
  }
  return {
    left: r.left * z,
    top: r.top * z,
    right: (window.innerWidth - r.right) * z,
    bottom: (window.innerHeight - r.bottom) * z,
    radius: radius * z,
  };
}

export function overlaps(a: DOMRect, b: DOMRect): boolean {
  return (
    a.width > 0 &&
    a.height > 0 &&
    a.left < b.right &&
    a.right > b.left &&
    a.top < b.bottom &&
    a.bottom > b.top
  );
}

/** Whether a portal (popover, menu, dialog…) lands over the slot. A portal's
 *  wrapper is an unstyled div, so its children are measured too. */
export function coveredBy(slot: HTMLElement): boolean {
  const page = slot.getBoundingClientRect();
  for (const portal of document.body.children) {
    if (portal.id === "root" || !(portal instanceof HTMLElement)) continue;
    for (const el of [portal, ...portal.children]) {
      if (overlaps(el.getBoundingClientRect(), page)) return true;
    }
  }
  return false;
}

import type { Viewport } from "@/lib/browser";

export function appZoom(): number {
  const z = parseFloat(
    document.documentElement.style.getPropertyValue("--app-zoom"),
  );
  return Number.isFinite(z) && z > 0 ? z : 1;
}

/** The slot as window insets in logical points (CSS px × page zoom). */
export function measure(slot: HTMLElement): Viewport {
  const z = appZoom();
  const r = slot.getBoundingClientRect();
  return {
    left: r.left * z,
    top: r.top * z,
    right: (window.innerWidth - r.right) * z,
    bottom: (window.innerHeight - r.bottom) * z,
  };
}

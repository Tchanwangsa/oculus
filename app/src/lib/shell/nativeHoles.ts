import { invoke } from "@tauri-apps/api/core";
import { isWindows } from "@/lib/platform";
import { bottomRadii, portalsOver } from "./holeGeometry";
import { sendRegions } from "./nativeRegions";

/**
 * Holes the app cuts through itself onto the native pages under it. Every
 * browser tab is a WKWebView Rust places below the app's
 * transparent webview (`app/src-tauri/src/shell/`); a slot shows its page only
 * while a hole is open over it. The holes are one `clip-path` on `#root`, so
 * portals, which are children of `body`, still draw over a page.
 *
 * Rust sends clicks inside a shown page's frame to the page. While a portal
 * overlaps an open hole the app reports itself blocked and keeps them, so a
 * click on the page under a popover just dismisses the popover.
 *
 * An overlay inside `#root` that must cover the pages (the fullscreen media
 * player) suspends the holes: no clip-path, and the app reports itself
 * blocked. Holes still open and close meanwhile and come back on release.
 *
 * Open a hole only once its page is placed, and close it before the page is
 * hidden or parked, or the bare window shows through for a frame.
 *
 * Windows keeps its pages on top instead and writes nothing to `#root`: the
 * same holes, minus the portals over them, become each page's window region
 * (`nativeRegions.ts`).
 */

export interface NativeHole {
  open: () => void;
  close: () => void;
}

const holes = new Set<HTMLElement>();
let written = "";
let blocked = false;
let frame = 0;
let suspended = 0;
let resizes: ResizeObserver | null = null;
let mutations: MutationObserver | null = null;

export function openHole(slot: HTMLElement): void {
  if (holes.has(slot)) return;
  holes.add(slot);
  if (holes.size === 1) watch();
  resizes?.observe(slot);
  update();
}

/** Synchronous, so the hole is gone before the caller asks Rust to move the
 *  page away. */
export function closeHole(slot: HTMLElement): void {
  if (!holes.delete(slot)) return;
  resizes?.unobserve(slot);
  if (holes.size === 0) unwatch();
  update();
}

/** Paints the app over every page and keeps its clicks until the returned
 *  release runs. Suspensions nest; the holes return once all are released. */
export function suspendHoles(): () => void {
  suspended++;
  update();
  let released = false;
  return () => {
    if (released) return;
    released = true;
    suspended--;
    update();
  };
}

function update(): void {
  if (isWindows) {
    sendRegions(suspended ? [] : [...holes]);
    return;
  }
  const root = document.getElementById("root");
  if (!root) return;
  const path = holes.size && !suspended ? holePath(root, [...holes]) : "";
  if (path !== written) {
    written = path;
    if (path) root.style.clipPath = path;
    else root.style.removeProperty("clip-path");
  }
  setBlocked(suspended > 0 || [...holes].some(coveredBy));
}

/** `#root`'s rect clockwise, then each hole anticlockwise: under `nonzero` a
 *  hole winds to zero and drops out, and nothing else changes. CSS px from
 *  `#root`'s own box; page zoom scales both alike. */
function holePath(root: HTMLElement, slots: HTMLElement[]): string {
  const box = root.getBoundingClientRect();
  const n = (v: number) => Math.round(v * 100) / 100;
  let d = `M0 0H${n(box.width)}V${n(box.height)}H0Z`;
  for (const slot of slots) {
    const r = slot.getBoundingClientRect();
    if (r.width <= 0 || r.height <= 0) continue;
    const [bl, br] = bottomRadii(slot, r);
    const left = n(r.left - box.left);
    const top = n(r.top - box.top);
    const right = n(r.right - box.left);
    const bottom = n(r.bottom - box.top);
    d += `M${left} ${top}V${n(bottom - bl)}`;
    if (bl) d += `A${bl} ${bl} 0 0 0 ${n(left + bl)} ${bottom}`;
    d += `H${n(right - br)}`;
    if (br) d += `A${br} ${br} 0 0 0 ${right} ${n(bottom - br)}`;
    d += `V${top}Z`;
  }
  return `path(nonzero, "${d}")`;
}

function setBlocked(next: boolean): void {
  if (next === blocked) return;
  blocked = next;
  invoke("native_set_blocked", { blocked: next }).catch(() => {});
}

/** Measured a frame after the change, once a popper has positioned. */
function schedule(): void {
  if (!frame) {
    frame = requestAnimationFrame(() => {
      frame = 0;
      update();
    });
  }
}

function watch(): void {
  resizes = new ResizeObserver(schedule);
  mutations = new MutationObserver(schedule);
  mutations.observe(document.body, {
    childList: true,
    subtree: true,
    attributes: true,
    attributeFilter: ["style", "data-state"],
  });
  window.addEventListener("resize", schedule);
  // A popover's open animation changes no attribute, and its rect mid-way
  // is scaled and shifted; measure again once it settles.
  document.addEventListener("animationend", schedule, true);
  document.addEventListener("transitionend", schedule, true);
}

function unwatch(): void {
  resizes?.disconnect();
  mutations?.disconnect();
  resizes = mutations = null;
  window.removeEventListener("resize", schedule);
  document.removeEventListener("animationend", schedule, true);
  document.removeEventListener("transitionend", schedule, true);
  if (frame) cancelAnimationFrame(frame);
  frame = 0;
}

/** Whether a portal (popover, menu, dialog…) lands over the slot. */
function coveredBy(slot: HTMLElement): boolean {
  return portalsOver(slot.getBoundingClientRect()).length > 0;
}

import { convertFileSrc } from "@tauri-apps/api/core";

/** The iframe's height before the page first reports, and the clamp after;
 *  taller content scrolls inside the frame. */
export const DEFAULT_HEIGHT = 320;
export const MIN_HEIGHT = 120;
export const MAX_HEIGHT = 560;

export const HEIGHT_MESSAGE = "oculus-embed-height";

/** Injected into the page: posts its content height to the parent on load and
 *  on every resize. The body's scroll height catches overflow that the root's
 *  box doesn't. */
export const REPORT_HEIGHT = `(function(){var last=0;function post(){var d=document.documentElement,b=document.body;var h=Math.ceil(Math.max(d.getBoundingClientRect().height,b?b.scrollHeight:0));if(h!==last){last=h;parent.postMessage({type:"${HEIGHT_MESSAGE}",height:h},"*");}}addEventListener("DOMContentLoaded",post);addEventListener("load",post);if(typeof ResizeObserver!=="undefined")new ResizeObserver(post).observe(document.documentElement);})();`;

/**
 * The page's directory as an asset URL ending in `/`, for `<base href>`.
 * `convertFileSrc` encodes slashes too, so the data dir becomes one segment
 * and the library path keeps real ones — `../` then resolves within the
 * library; the asset protocol percent-decodes the whole path either way.
 */
export function baseHref(dataDir: string, path: string): string {
  const dirs = path.split("/").slice(0, -1).filter(Boolean);
  return `${convertFileSrc(dataDir)}/${dirs.map((d) => `${encodeURIComponent(d)}/`).join("")}`;
}

export const escapeAttr = (s: string) => s.replace(/&/g, "&amp;").replace(/"/g, "&quot;");

/**
 * The page with a `<base>`, a colour scheme and the height reporter at the
 * top of its `<head>` (one is opened if absent). The scheme is the app's own,
 * not `light dark`, because the app themes by class and the OS may disagree;
 * a page that declares one keeps it.
 */
export function prepare(html: string, base: string, dark: boolean): string {
  let tags = "";
  if (!/<base[\s>]/i.test(html)) tags += `<base href="${escapeAttr(base)}">`;
  if (!/<meta[^>]+name\s*=\s*["']?color-scheme|color-scheme\s*:/i.test(html))
    tags += `<meta name="color-scheme" content="${dark ? "dark" : "light"}">`;
  tags += `<script>${REPORT_HEIGHT}</script>`;

  const head = /<head(\s[^>]*)?>/i.exec(html);
  if (head) return splice(html, head.index + head[0].length, tags);
  // After `<html>`, else after a doctype (before it would mean quirks mode).
  const opener = /<html(\s[^>]*)?>/i.exec(html) ?? /<!doctype[^>]*>/i.exec(html);
  return splice(html, opener ? opener.index + opener[0].length : 0, `<head>${tags}</head>`);
}

export const splice = (s: string, at: number, insert: string) => s.slice(0, at) + insert + s.slice(at);

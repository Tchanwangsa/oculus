import { convertFileSrc } from "@tauri-apps/api/core";

/** Heights are CSS px inside the frame; native page zoom needs no conversion. */
export const DEFAULT_HEIGHT = 320;
export const MIN_HEIGHT = 120;
// About 178 screens at 560 px: room for long pages, but finite layout work
// if viewport-dependent growth doesn't follow the constant-offset rule below.
export const RUNAWAY_HEIGHT_CEILING = 100_000;

export type PageHeightReport = {
  height: number;
  viewportHeight: number;
  viewportWidth: number;
  revision: number;
};

type PageHeight = {
  height: number;
  viewportHeight: number | null;
  viewportWidth: number | null;
  revision: number | null;
  delta: number;
  growthSteps: number;
  held: boolean;
};

export const initialPageHeight = (height = DEFAULT_HEIGHT): PageHeight => ({
  height, viewportHeight: null, viewportWidth: null, revision: null,
  delta: 0, growthSteps: 0, held: false,
});

/** Count matching growth only across actual viewport resizes with unchanged
 *  content. Content revisions and width changes release a hold, so new rows
 *  can grow equally without being mistaken for viewport feedback. */
export function nextPageHeight(state: PageHeight, report: PageHeightReport): PageHeight {
  if (!Number.isFinite(report.height)) return state;
  const height = Math.min(RUNAWAY_HEIGHT_CEILING, Math.max(MIN_HEIGHT, Math.ceil(report.height)));
  const measured = {
    viewportHeight: report.viewportHeight,
    viewportWidth: report.viewportWidth,
    revision: report.revision,
  };
  if (report.revision !== state.revision || report.viewportWidth !== state.viewportWidth || height < state.height) {
    return { ...initialPageHeight(height), ...measured };
  }
  if (state.held) return state;
  if (height === state.height && report.viewportHeight === state.viewportHeight) return state;
  const delta = height - state.height;
  const followsResize = state.viewportHeight !== null && report.viewportHeight > state.viewportHeight
    && Math.abs(report.viewportHeight - state.height) <= 1;
  const growthSteps = delta > 0 && followsResize
    ? (Math.abs(delta - state.delta) <= 1 ? state.growthSteps + 1 : 1)
    : 0;
  // Three matching resize responses identify viewport + constant feedback.
  // Holding before the third response breaks the loop; other loops hit the ceiling.
  if (growthSteps >= 3) return { ...state, held: true };
  return { height, ...measured, delta, growthSteps, held: false };
}

export const HEIGHT_MESSAGE = "oculus-embed-height";

/** Expanded copies and other embeds share the message type, never the source. */
export function readPageHeight(
  event: Pick<MessageEvent, "source" | "data">,
  source: Window | null,
): PageHeightReport | null {
  if (!source || event.source !== source || event.data?.type !== HEIGHT_MESSAGE) return null;
  const { height, viewportHeight, viewportWidth, revision } = event.data;
  if (![height, viewportHeight, viewportWidth].every((value) => typeof value === "number" && Number.isFinite(value))) return null;
  if (viewportHeight <= 0 || viewportWidth <= 0 || !Number.isSafeInteger(revision) || revision < 0) return null;
  return { height, viewportHeight, viewportWidth, revision };
}

/** Report viewport measurements even when height is unchanged, so the parent
 *  can distinguish a resize response from later content growth. Revisions
 *  track content changes independently of the frame's own dimensions. */
export const REPORT_HEIGHT = `(function(){
  var last="", revision=0, d=document.documentElement;
  function post(){
    var b=document.body;
    var height=Math.ceil(Math.max(d.getBoundingClientRect().height,b?b.scrollHeight:0));
    var key=[height,innerHeight,innerWidth,revision].join(":");
    if(key===last)return;
    last=key;
    parent.postMessage({type:"${HEIGHT_MESSAGE}",height:height,
      viewportHeight:innerHeight,viewportWidth:innerWidth,revision:revision},"*");
  }
  function changed(){revision++;post();}
  addEventListener("DOMContentLoaded",changed);
  addEventListener("resize",post);
  ["load","error","input","change","animationend","animationcancel","transitionend","transitioncancel"].forEach(function(type){
    addEventListener(type,changed,true);
  });
  if(typeof ResizeObserver!=="undefined"){
    var observer=new ResizeObserver(post);
    observer.observe(d);
    addEventListener("DOMContentLoaded",function(){if(document.body)observer.observe(document.body);});
  }
  if(typeof MutationObserver!=="undefined")new MutationObserver(changed).observe(d,{
    subtree:true,childList:true,attributes:true,characterData:true
  });
  if(document.fonts)document.fonts.addEventListener("loadingdone",changed);
})();`;

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
 * a page that declares one keeps it. Only the inline copy suppresses document
 * overflow: the chat scrolls, and a rounded pixel mustn't add a classic scrollbar.
 */
export function prepare(html: string, base: string, dark: boolean, inline = false): string {
  let tags = "";
  if (!/<base[\s>]/i.test(html)) tags += `<base href="${escapeAttr(base)}">`;
  if (!/<meta[^>]+name\s*=\s*["']?color-scheme|color-scheme\s*:/i.test(html))
    tags += `<meta name="color-scheme" content="${dark ? "dark" : "light"}">`;
  if (inline) tags += `<style>html,body{overflow:hidden!important}</style>`;
  tags += `<script>${REPORT_HEIGHT}</script>`;

  const head = /<head(\s[^>]*)?>/i.exec(html);
  if (head) return splice(html, head.index + head[0].length, tags);
  // After `<html>`, else after a doctype (before it would mean quirks mode).
  const opener = /<html(\s[^>]*)?>/i.exec(html) ?? /<!doctype[^>]*>/i.exec(html);
  return splice(html, opener ? opener.index + opener[0].length : 0, `<head>${tags}</head>`);
}

export const splice = (s: string, at: number, insert: string) => s.slice(0, at) + insert + s.slice(at);

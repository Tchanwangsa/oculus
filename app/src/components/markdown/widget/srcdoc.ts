import { DEFAULT_BINDING, KIT_FLAG, WIDGET_MESSAGE } from "../../../../widget-kit/protocol";

/**
 * The document a widget frame loads (docs/widgets.md#the-sandbox): a CSP
 * that lets in only the kit's own files and this document's nonce, the
 * import map, the theme's tokens, the guard script, the saved state and the
 * widget's module. Pure, so its escaping and policy are tested.
 */
export interface SrcdocInput {
  /** The kit's URL, ending in `/` (`http://127.0.0.1:<port>/widget-kit/`). */
  kitBase: string;
  /** Fresh per document; every inline script carries it. */
  nonce: string;
  /** The transpiled widget: an ES module that binds its component to `DEFAULT_BINDING`. */
  js: string;
  state: Record<string, unknown>;
  /** `--color-…` custom properties for the current theme. */
  tokens: Record<string, string>;
  dark: boolean;
}

/** What a widget may import, and where each resolves. */
export function importMap(kitBase: string) {
  return {
    imports: {
      react: `${kitBase}react.js`,
      "react/jsx-runtime": `${kitBase}jsx-runtime.js`,
      "oculus/ui": `${kitBase}ui.js`,
      "oculus/state": `${kitBase}state.js`,
      "oculus/2d": `${kitBase}2d.js`,
      "oculus/math": `${kitBase}math.js`,
      "oculus/3d": `${kitBase}3d.js`,
      "oculus/ask": `${kitBase}ask.js`,
      three: `${kitBase}three.js`,
      "@react-three/fiber": `${kitBase}fiber.js`,
    },
  };
}

/** The policy, minimal as measured: the kit's path for scripts, styles,
 *  fonts and the maths wasm fetch, inline scripts only by nonce, inline
 *  styles (the maths engine sets `style=`), pictures only from data/blob. */
export function widgetCsp(kitBase: string, nonce: string): string {
  return [
    "default-src 'none'",
    `script-src ${kitBase} 'nonce-${nonce}' 'wasm-unsafe-eval'`,
    `style-src ${kitBase} 'unsafe-inline'`,
    `font-src ${kitBase} data:`,
    "img-src data: blob:",
    `connect-src ${kitBase}`,
  ].join("; ");
}

/** JS safe inside `<script>`: `</script` would close the element and `<!--`
 *  would switch the parser into its escaped state. Both rewrites mean the
 *  same inside a string, template, regex or comment, the only places they
 *  can occur in valid JS. */
export function escapeScript(js: string): string {
  return js.replace(/<\/(script)/gi, "<\\/$1").replace(/<!--/g, "<\\!--");
}

/** JSON safe inside `<script>`: every `<` as its escape. */
export const scriptJson = (value: unknown) => JSON.stringify(value).replace(/</g, "\\u003c");

const escapeAttr = (s: string) => s.replace(/&/g, "&amp;").replace(/"/g, "&quot;").replace(/</g, "&lt;");

/** Only custom properties named like tokens, with values that can't leave
 *  the declaration. */
export function tokenCss(tokens: Record<string, string>): string {
  const decls = Object.entries(tokens)
    .filter(([name, value]) => /^--[a-z0-9-]+$/.test(name) && /^[#\w\s(),.%/-]+$/.test(value))
    .map(([name, value]) => `${name}:${value.trim()}`);
  return `:root{${decls.join(";")}}`;
}

/**
 * Runs first, before any module: turns every way a widget can fail outside
 * React into an `error` message — a module that won't load (a kit file or a
 * blocked import), a throw in a handler or timer, a rejection, a CSP block —
 * and on `load` reports a kit that never evaluated. Classic, so it runs even
 * when no module can.
 */
export const GUARD = `(function(){var T=${JSON.stringify(WIDGET_MESSAGE)};function post(m){m.type=T;try{parent.postMessage(m,"*")}catch(_){}}function text(e){return e&&e.message?(e.name&&e.name!=="Error"?e.name+": ":"")+e.message:String(e)}addEventListener("error",function(e){var t=e.target;if(t&&t!==window&&t.tagName){var k=t.tagName==="SCRIPT"||t.tagName==="LINK";post({kind:"error",phase:k?"kit":"runtime",message:"Couldn't load "+(t.src||t.href||(k?"the widget's modules":"a resource"))});return}post({kind:"error",phase:"runtime",message:e.error?text(e.error):String(e.message||"Script error")})},true);addEventListener("unhandledrejection",function(e){post({kind:"error",phase:"runtime",message:"Unhandled rejection: "+text(e.reason)})});addEventListener("securitypolicyviolation",function(e){post({kind:"error",phase:"csp",message:"The sandbox blocked "+(e.blockedURI||"an inline resource")+" ("+e.violatedDirective+")"})});addEventListener("load",function(){if(!window[${JSON.stringify(KIT_FLAG)}])post({kind:"error",phase:"kit",message:"The widget kit did not load"})})})();`;

export function buildSrcdoc({ kitBase, nonce, js, state, tokens, dark }: SrcdocInput): string {
  const n = escapeAttr(nonce);
  const mount = `\nimport { mount as __oculusMount } from ${JSON.stringify(`${kitBase}runtime.js`)};\n__oculusMount(${DEFAULT_BINDING});\n`;
  return [
    "<!doctype html>",
    '<html><head><meta charset="utf-8">',
    `<meta http-equiv="Content-Security-Policy" content="${escapeAttr(widgetCsp(kitBase, nonce))}">`,
    `<meta name="color-scheme" content="${dark ? "dark" : "light"}">`,
    `<script nonce="${n}">${GUARD}</script>`,
    `<script type="importmap" nonce="${n}">${scriptJson(importMap(kitBase))}</script>`,
    `<link rel="stylesheet" data-oculus-kit href="${escapeAttr(`${kitBase}kit.css`)}">`,
    `<style>${tokenCss(tokens)}</style>`,
    `<script type="application/json" id="oculus-state">${scriptJson(state)}</script>`,
    '</head><body><div id="root"></div>',
    `<script type="module" nonce="${n}">${escapeScript(js + mount)}</script>`,
    "</body></html>",
  ].join("");
}

/** A nonce for one document. */
export function newNonce(): string {
  const bytes = crypto.getRandomValues(new Uint8Array(16));
  return btoa(String.fromCharCode(...bytes)).replace(/[+/=]/g, "");
}

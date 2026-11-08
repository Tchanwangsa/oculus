import { afterAll, describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { renderToStaticMarkup } from "react-dom/server";
import ReactMarkdown from "react-markdown";

// The fence's imports reach modules that listen on `window` and `document`
// at load; tests run without a DOM (as in openui.test.tsx).
const globals = globalThis as { window?: unknown; document?: unknown };
const stubbed = (["window", "document"] as const).filter((k) => !(k in globals));
for (const k of stubbed) globals[k] = Object.assign(new EventTarget(), { documentElement: { style: {} } });
afterAll(() => {
  for (const k of stubbed) delete globals[k];
});
const { MD_COMPONENTS } = await import("@/components/markdown/MdComponents");
const { remarkWidgets } = await import("@/components/markdown/openui/remarkWidgets");
const { widgetTitle } = await import("@/components/markdown/widget/title");
const { buildSrcdoc, escapeScript, importMap, tokenCss, widgetCsp } = await import("@/components/markdown/widget/srcdoc");
const { readWidgetMessage, sendReply, sendState, sendVisibility, visibilityToPost } = await import("@/components/markdown/widget/messages");
const { fixRequest } = await import("@/components/markdown/widget/fixRequest");
const { IN_VIEW_SETTLE_MS, settleInView } = await import("@/components/markdown/widget/useFrameInView");

const { DEFAULT_BINDING } = await import("../../../widget-kit/protocol");

const KIT = "http://127.0.0.1:4321/widget-kit/";
const WIDGET = `// title: Phase on a circle
import { useState } from "react";
export default function Phase() { return null; }`;

describe("widget fences", () => {
  test("share one counter with openui fences", () => {
    const md = "```openui\nroot = Stack([])\n```\n\n```widget\n// title: A\n```\n\n```ts\nx\n```\n\n```openui-lang\nroot = Stack([])\n```\n\n```widget\n// title: B\n```\n";
    const html = renderToStaticMarkup(<ReactMarkdown remarkPlugins={[remarkWidgets]}>{md}</ReactMarkdown>);
    const indices = [...html.matchAll(/data-widget-index="(\d+)"/g)].map((m) => Number(m[1]));
    expect(indices).toEqual([0, 1, 2, 3]);
    expect(html).toContain('class="language-widget" data-widget-index="1"');
    expect(html).toContain('class="language-widget" data-widget-index="3"');
  });

  test("outside a committed row it only says it is building", () => {
    const md = "Look:\n\n```widget\n" + WIDGET + "\n```\n";
    const html = renderToStaticMarkup(
      <ReactMarkdown remarkPlugins={[remarkWidgets]} components={MD_COMPONENTS}>
        {md}
      </ReactMarkdown>,
    );
    expect(html).toContain("Building Phase on a circle…");
    expect(html).not.toContain("<iframe");
    // The source rides only in `data-md`, for copying the reply as markdown.
    expect(html).toContain("data-md=");
    expect(html.replace(/data-md="[^"]*"/, "")).not.toContain("export default");
  });

  test("a fence with no line yet is building too, not code", () => {
    const html = renderToStaticMarkup(<ReactMarkdown components={MD_COMPONENTS}>{"```widget\n"}</ReactMarkdown>);
    expect(html).toContain("Building Widget…");
    expect(html).not.toContain("<pre");
  });
});

describe("widgetTitle", () => {
  test("reads the first line's title", () => {
    expect(widgetTitle("// title: Eigenphase\nexport default () => null")).toBe("Eigenphase");
    expect(widgetTitle("\n//title:   Spaced  \n")).toBe("Spaced");
    expect(widgetTitle("// Title: Case\n")).toBe("Case");
  });
  test("falls back to Widget", () => {
    expect(widgetTitle("import x from 'react';\n// title: late")).toBe("Widget");
    expect(widgetTitle("// title:   ")).toBe("Widget");
    expect(widgetTitle("")).toBe("Widget");
  });
});

describe("buildSrcdoc", () => {
  const doc = buildSrcdoc({
    kitBase: KIT,
    nonce: "n0nce",
    js: 'const s = "</script><script>alert(1)</script>";\n// <!-- x\nfunction W() { return s; }\nconst __oculusWidget = W;\n',
    state: { note: "</script><b>", n: 2 },
    tokens: { "--color-card": "#212121", "--color-foreground": "#ececec", "--color-bad": "red;}</style><script>", "bad name": "#fff" },
    dark: true,
  });

  test("a streaming frame boots the runtime with no program, under the same policy", () => {
    const stream = buildSrcdoc({ kitBase: KIT, nonce: "n0nce", js: null, state: {}, tokens: {}, dark: false });
    expect(stream).toContain(`import { stream as __oculusStream } from "${KIT}runtime.js";`);
    expect(stream).not.toContain("__oculusMount");
    expect(stream).not.toContain(DEFAULT_BINDING);
    expect(stream).toContain(`<meta http-equiv="Content-Security-Policy" content="${widgetCsp(KIT, "n0nce")}">`);
  });

  test("the guard keeps a preview's throws quiet, never a file that won't load", () => {
    expect(doc).toContain('if(window["__oculusQuiet"])return;post({kind:"error",phase:"runtime"');
    expect(doc).toContain('if(window["__oculusQuiet"])return;post({kind:"error",phase:"runtime",message:"Unhandled rejection: "');
    expect(doc.indexOf("Couldn't load ")).toBeLessThan(doc.indexOf("__oculusQuiet"));
  });

  test("the CSP names the kit and the nonce, nothing wider", () => {
    const csp = widgetCsp(KIT, "n0nce");
    expect(csp).toStartWith("default-src 'none'");
    expect(csp).toContain(`script-src ${KIT} 'nonce-n0nce' 'wasm-unsafe-eval'`);
    expect(csp).toContain(`style-src ${KIT} 'unsafe-inline'`);
    expect(csp).toContain(`font-src ${KIT} data:`);
    expect(csp).toContain(`connect-src ${KIT}`);
    expect(csp).not.toContain(" 'unsafe-eval'");
    expect(csp).not.toContain("*");
    expect(doc).toContain(`<meta http-equiv="Content-Security-Policy" content="${csp}">`);
  });

  test("the import map maps only what a widget may import", () => {
    expect(Object.keys(importMap(KIT).imports).sort()).toEqual([
      "@react-three/fiber",
      "oculus/2d",
      "oculus/3d",
      "oculus/ask",
      "oculus/math",
      "oculus/state",
      "oculus/ui",
      "react",
      "react/jsx-runtime",
      "three",
    ]);
    expect(doc).toContain('<script type="importmap" nonce="n0nce">');
    expect(doc).toContain(`"oculus/ui":"${KIT}ui.js"`);
    expect(doc).toContain(`"oculus/2d":"${KIT}2d.js"`);
    expect(doc).toContain(`"oculus/math":"${KIT}math.js"`);
    expect(doc).toContain(`"oculus/3d":"${KIT}3d.js"`);
    expect(doc).toContain(`"oculus/ask":"${KIT}ask.js"`);
    expect(doc).toContain(`"three":"${KIT}three.js"`);
    expect(doc).toContain(`"@react-three/fiber":"${KIT}fiber.js"`);
    // Before any module script, or it doesn't apply.
    expect(doc.indexOf("importmap")).toBeLessThan(doc.indexOf('type="module"'));
  });

  test("the import map and the transpiler's allow-list name the same modules", () => {
    const rust = readFileSync(join(import.meta.dir, "../../../src-tauri/src/widgets/transpile.rs"), "utf8");
    const list = rust.match(/ALLOWED_IMPORTS: &\[&str\] = &\[([^\]]*)\]/)?.[1] ?? "";
    const allowed = [...list.matchAll(/"([^"]+)"/g)].map((m) => m[1]).sort();
    expect(allowed).toEqual(Object.keys(importMap(KIT).imports).sort());
  });

  test("every script carries the nonce", () => {
    const start = doc.indexOf('<script type="module"');
    const tags = doc.slice(0, doc.indexOf(">", start) + 1);
    const scripts = [...tags.matchAll(/<script(?![^>]*application\/json)([^>]*)>/g)];
    expect(scripts.length).toBe(3);
    for (const s of scripts) expect(s[1]).toContain('nonce="n0nce"');
  });

  test("the widget can't close its script element", () => {
    const module = doc.slice(doc.indexOf('<script type="module"'));
    expect(module.match(/<\/script/gi)?.length).toBe(1);
    expect(module).toContain("<\\/script><script>alert(1)<\\/script>");
    expect(module).toContain("<\\!-- x");
    expect(escapeScript("a</SCRIPT b")).toBe("a<\\/SCRIPT b");
  });

  test("the module mounts the transpiler's default binding through the runtime", () => {
    expect(DEFAULT_BINDING).toBe("__oculusWidget");
    expect(doc).toContain(`import { mount as __oculusMount } from "${KIT}runtime.js";`);
    expect(doc).toContain(`__oculusMount(${DEFAULT_BINDING});`);
  });

  test("the state is embedded as JSON that can't close its element", () => {
    const m = /<script type="application\/json" id="oculus-state">(.*?)<\/script>/.exec(doc);
    expect(m).not.toBeNull();
    expect(m![1]).not.toContain("<");
    expect(JSON.parse(m![1])).toEqual({ note: "</script><b>", n: 2 });
  });

  test("tokens, scheme and kit stylesheet", () => {
    expect(doc).toContain('<meta name="color-scheme" content="dark">');
    expect(doc).toContain(`<link rel="stylesheet" data-oculus-kit href="${KIT}kit.css">`);
    expect(doc).toContain("--color-card:#212121");
    expect(tokenCss({ "--color-bad": "red;}</style>", "x": "#fff" })).toBe(":root{}");
    expect(doc).not.toContain("--color-bad");
  });
});

describe("readWidgetMessage", () => {
  const frame = {} as Window;
  const other = {} as Window;
  const msg = (data: unknown, source: unknown = frame) => readWidgetMessage({ source, data }, frame);

  test("takes this frame's messages", () => {
    expect(msg({ type: "oculus-widget", kind: "height", height: 240.2 })).toEqual({ type: "oculus-widget", kind: "height", height: 241 });
    expect(msg({ type: "oculus-widget", kind: "state", state: { a: 1 } })?.kind).toBe("state");
    expect(msg({ type: "oculus-widget", kind: "error", phase: "render", message: "boom" })?.kind).toBe("error");
    expect(msg({ type: "oculus-widget", kind: "overflow", width: 500, frameWidth: 400 })?.kind).toBe("overflow");
  });

  test("rejects other sources, other types and bad shapes", () => {
    expect(msg({ type: "oculus-widget", kind: "height", height: 10 }, other)).toBeNull();
    expect(msg({ type: "oculus-widget", kind: "height", height: 10 }, null)).toBeNull();
    expect(readWidgetMessage({ source: frame, data: { type: "oculus-widget", kind: "height", height: 1 } }, null)).toBeNull();
    expect(msg({ type: "oculus-embed-height", height: 10 })).toBeNull();
    expect(msg({ type: "oculus-widget", kind: "height", height: "10" })).toBeNull();
    expect(msg({ type: "oculus-widget", kind: "height", height: Infinity })).toBeNull();
    expect(msg({ type: "oculus-widget", kind: "state", state: [1] })).toBeNull();
    expect(msg({ type: "oculus-widget", kind: "error", phase: "parent", message: "x" })).toBeNull();
    expect(msg({ type: "oculus-widget", kind: "navigate", to: "x" })).toBeNull();
    expect(msg("oculus-widget")).toBeNull();
  });

  test("takes oculus/ask's requests in exactly their shapes", () => {
    const ask = { type: "oculus-widget", kind: "ask", id: "r1", prompt: "Steps?", schema: { type: "object" } };
    expect(msg(ask)).toEqual(ask);
    expect(msg({ ...ask, extra: 1 })).toEqual(ask);
    const send = { type: "oculus-widget", kind: "send", id: "r2", text: "Explain it" };
    expect(msg(send)).toEqual(send);
    // Over-long text is the gate's to refuse with a reply, so it passes here.
    expect(msg({ ...send, text: "x".repeat(10_000) })?.kind).toBe("send");
    expect(msg(ask, other)).toBeNull();
    for (const bad of [
      { ...ask, id: 1 },
      { ...ask, id: "" },
      { ...ask, id: "has space" },
      { ...ask, id: "x".repeat(65) },
      { ...ask, prompt: 3 },
      { ...ask, schema: [1] },
      { ...ask, schema: "object" },
      { ...ask, schema: null },
      { ...send, text: null },
      { ...send, id: undefined },
    ]) {
      expect(msg(bad)).toBeNull();
    }
  });
});

describe("messages to a frame", () => {
  const frame = () => {
    const posted: { message: unknown; origin: string }[] = [];
    return { posted, window: { postMessage: (message: unknown, origin: string) => posted.push({ message, origin }) } as unknown as Window };
  };

  test("pause, resume and state are the host's messages", () => {
    const f = frame();
    sendVisibility(f.window, true);
    sendVisibility(f.window, false);
    sendState(f.window, { a: 1 });
    expect(f.posted).toEqual([
      { message: { type: "oculus-host", kind: "resume" }, origin: "*" },
      { message: { type: "oculus-host", kind: "pause" }, origin: "*" },
      { message: { type: "oculus-host", kind: "state", state: { a: 1 } }, origin: "*" },
    ]);
    // A frame not yet attached is skipped.
    expect(() => sendVisibility(null, true)).not.toThrow();
  });

  test("a reply echoes the request's id with its value or its error", () => {
    const f = frame();
    sendReply(f.window, "r1", { value: { steps: ["a"] } });
    sendReply(f.window, "r2", { error: "A request is already running." });
    sendReply(f.window, "r3", { value: null });
    expect(f.posted.map((p) => p.message)).toEqual([
      { type: "oculus-host", kind: "reply", id: "r1", ok: true, value: { steps: ["a"] } },
      { type: "oculus-host", kind: "reply", id: "r2", ok: false, error: "A request is already running." },
      { type: "oculus-host", kind: "reply", id: "r3", ok: true, value: null },
    ]);
    expect(() => sendReply(undefined, "r4", { value: 1 })).not.toThrow();
  });

  test("visibility is posted after the first height, then only when it changes", () => {
    // Before the frame's runtime listens, nothing.
    expect(visibilityToPost(null, true, false)).toBeNull();
    expect(visibilityToPost(null, false, false)).toBeNull();
    // Its first report: the flag as it is, paused included.
    expect(visibilityToPost(null, true, true)).toBe(true);
    expect(visibilityToPost(null, false, true)).toBe(false);
    // Then changes only.
    expect(visibilityToPost(true, true, true)).toBeNull();
    expect(visibilityToPost(true, false, true)).toBe(false);
    expect(visibilityToPost(false, true, true)).toBe(true);
  });
});

test("a frame is in view only once it has stayed in; leaving is at once", () => {
  const seen: boolean[] = [];
  const timers = new Map<number, () => void>();
  let next = 0;
  const settle = settleInView(
    (v) => seen.push(v),
    (fn, ms) => {
      expect(ms).toBe(IN_VIEW_SETTLE_MS);
      timers.set(++next, fn);
      return next;
    },
    (h) => timers.delete(h as number),
  );
  const fire = () => {
    for (const [handle, fn] of timers) {
      timers.delete(handle);
      fn();
    }
  };
  // Passed through before it settled: no context.
  settle.update(true);
  settle.update(true);
  expect(timers.size).toBe(1);
  settle.update(false);
  expect(timers.size).toBe(0);
  expect(seen).toEqual([false]);
  // Stayed in: in view once the timer fires, then out at once.
  settle.update(true);
  fire();
  settle.update(false);
  expect(seen).toEqual([false, true, false]);
  // A pending entry is dropped on cancel (element change, unmount).
  settle.update(true);
  settle.cancel();
  expect(timers.size).toBe(0);
  expect(seen).toEqual([false, true, false]);
});

test("Ask to fix sends the error and the fence", () => {
  expect(fixRequest("Phase", "3:4 Unexpected token.", "// title: Phase\nx")).toBe(
    'The widget "Phase" failed: 3:4 Unexpected token. Fix it and resend the widget.\n\n```widget\n// title: Phase\nx\n```',
  );
});

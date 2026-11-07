import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import * as React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import ts from "typescript";
import { importMap } from "@/components/markdown/widget/srcdoc";
import * as KitReact from "../../widget-kit/react";
import * as KitJsx from "../../widget-kit/jsx-runtime";
import * as UI from "../../widget-kit/ui";
import * as State from "../../widget-kit/state";
import * as Plane2d from "../../widget-kit/2d";
import * as Maths from "../../widget-kit/math";
import * as Space from "../../widget-kit/3d";
import * as Ask from "../../widget-kit/ask";
import { PlaneView } from "../../widget-kit/2d/Plane";
import { katexShim, type Target } from "../../widget-kit/2d/katex";
import { getState, onPublish, POST_DEBOUNCE_MS, replaceState, setValue } from "../../widget-kit/state/store";

const KIT = join(import.meta.dir, "../../widget-kit");

/** The values `declare module "<name>"` exports in widget.d.ts: its
 *  functions (each name once, overloads included) and constants. */
function declared(module: string): string[] {
  const file = ts.createSourceFile("widget.d.ts", readFileSync(join(KIT, "widget.d.ts"), "utf8"), ts.ScriptTarget.Latest, true);
  const names: string[] = [];
  for (const stmt of file.statements) {
    if (!ts.isModuleDeclaration(stmt) || stmt.name.getText().slice(1, -1) !== module) continue;
    const body = stmt.body as ts.ModuleBlock;
    for (const s of body.statements) {
      const exported = ts.getCombinedModifierFlags(s as ts.Declaration) & ts.ModifierFlags.Export;
      if (!exported) continue;
      if (ts.isFunctionDeclaration(s) && s.name) names.push(s.name.text);
      if (ts.isVariableStatement(s)) for (const d of s.declarationList.declarations) names.push(d.name.getText());
    }
  }
  return [...new Set(names)].sort();
}

describe("kit entries", () => {
  test("react re-exports every public name React has", () => {
    // Internals, test-only and server-only names a widget never needs.
    const skip = new Set(["act", "cacheSignal", "captureOwnerStack", "unstable_useCacheRefresh"]);
    const missing = Object.keys(React).filter((k) => !k.startsWith("__") && !skip.has(k) && !(k in KitReact));
    expect(missing).toEqual([]);
    expect(KitReact.default.useState).toBe(React.useState);
  });

  test("jsx-runtime has what the automatic runtime imports", () => {
    expect(Object.keys(KitJsx).sort()).toEqual(["Fragment", "jsx", "jsxs"]);
  });

  test("widget.d.ts is the contract for every oculus module", () => {
    expect(declared("oculus/ui")).toEqual(Object.keys(UI).sort());
    expect(declared("oculus/state")).toEqual(Object.keys(State).sort());
    expect(declared("oculus/2d")).toEqual(Object.keys(Plane2d).sort());
    expect(declared("oculus/math")).toEqual(Object.keys(Maths).sort());
    expect(declared("oculus/3d")).toEqual(Object.keys(Space).sort());
    expect(declared("oculus/ask")).toEqual(Object.keys(Ask).sort());
  });

  test("the import map names each module the d.ts declares, plus react, three and r3f", () => {
    const names = Object.keys(importMap("x/").imports).sort();
    const modules = [...readFileSync(join(KIT, "widget.d.ts"), "utf8").matchAll(/declare module "([^"]+)"/g)].map((m) => m[1]);
    expect(names).toEqual([...modules, "react", "react/jsx-runtime", "three", "@react-three/fiber"].sort());
  });

  test("the kit build gives 2d, math, 3d, three and fiber entries of their own and aliases katex", () => {
    const config = readFileSync(join(KIT, "../vite.widget-kit.config.ts"), "utf8");
    expect(config).toContain('"2d": path.join(kit, "2d/index.ts")');
    expect(config).toContain('math: path.join(kit, "math/index.ts")');
    expect(config).toContain('"3d": path.join(kit, "3d/index.ts")');
    expect(config).toContain('three: path.join(kit, "three.ts")');
    expect(config).toContain('fiber: path.join(kit, "fiber.ts")');
    expect(config).toContain('ask: path.join(kit, "ask/index.ts")');
    expect(config).toContain('find: /^katex$/, replacement: path.join(kit, "2d/katex.ts")');
  });
});

describe("Pages and Tabs", () => {
  test("every page is laid out in one cell, only the current one visible", () => {
    const html = renderToStaticMarkup(
      <UI.Pages page={1}>
        <p>one</p>
        <p>two</p>
        <p>three</p>
      </UI.Pages>,
    );
    const panels = [...html.matchAll(/<div class="o-panel"([^>]*)>/g)].map((m) => m[1]);
    expect(panels.length).toBe(3);
    expect(panels.filter((p) => p.includes("visibility:hidden")).length).toBe(2);
    expect(panels[1]).not.toContain("visibility");
    expect(panels[0]).toContain("inert");
    expect(html).toContain("one");
    expect(html).toContain("three");
    expect(html.match(/class="o-dot"/g)?.length).toBe(3);
    expect(html).toMatch(/aria-selected="true" aria-label="Page 2 of 3"/);
  });

  test("a page out of range is clamped; one page has no controls", () => {
    const html = renderToStaticMarkup(
      <UI.Pages page={9}>
        <p>a</p>
        <p>b</p>
      </UI.Pages>,
    );
    const panels = [...html.matchAll(/<div class="o-panel"([^>]*)>/g)].map((m) => m[1]);
    expect(panels[1]).not.toContain("visibility");
    expect(renderToStaticMarkup(<UI.Pages><p>only</p></UI.Pages>)).not.toContain("o-pages-nav");
  });

  test("tabs stack their panels the same way", () => {
    const html = renderToStaticMarkup(
      <UI.Tabs labels={["Plot", "Table"]}>
        <p>plot</p>
        <p>table</p>
      </UI.Tabs>,
    );
    expect(html.match(/visibility:hidden/g)?.length).toBe(1);
    expect(html).toContain('aria-selected="true" class="o-tab">Plot');
  });

  test("Value uses tabular figures through its class, never a mono font", () => {
    const html = renderToStaticMarkup(<UI.Value label="Balance" value={1234.5} digits={2} unit="$" />);
    expect(html).toContain("o-value-number");
    expect(html).not.toContain("mono");
    const css = readFileSync(join(KIT, "kit.css"), "utf8");
    expect(css).toContain("font-variant-numeric: tabular-nums");
    expect(css).not.toMatch(/monospace|#[0-9a-f]{3,6}\b/i);
  });
});

describe("widget state", () => {
  test("a change is published whole, once it rests", async () => {
    const posts: Record<string, unknown>[] = [];
    onPublish((s) => posts.push(s));
    replaceState({ a: 1 });
    setValue("b", 1);
    setValue("b", 2);
    expect(getState()).toEqual({ a: 1, b: 2 });
    expect(posts).toEqual([]);
    await new Promise((r) => setTimeout(r, POST_DEBOUNCE_MS + 50));
    expect(posts).toEqual([{ a: 1, b: 2 }]);
    replaceState({ c: 3 });
    await new Promise((r) => setTimeout(r, POST_DEBOUNCE_MS + 50));
    expect(posts.length).toBe(1);
    onPublish(null);
  });
});

/** A plane at a fixed width, as `Plane` draws it once measured. */
const plane = (children: React.ReactNode, props: Record<string, unknown> = {}) =>
  renderToStaticMarkup(
    <PlaneView width={400} height={300} {...props}>
      {children}
    </PlaneView>,
  );

describe("oculus/2d", () => {
  test("a plane is an SVG with axes, a grid and number ticks, at its height", () => {
    const html = plane(null);
    expect(html).toContain('<div class="MafsView" style="width:400px;height:300px" tabindex="-1">');
    expect(html).toMatch(/<svg width="400" height="300"/);
    expect(html).toContain('stroke="var(--mafs-line-color)"');
    expect(html).toContain('stroke="var(--mafs-origin-color)"');
    expect(html).toContain(`>\u22122<tspan visibility="hidden">\u2212</tspan></text>`);
    // Panning is off, so touch scrolls the chat.
    expect(html).toContain("touch-action:auto");
  });

  test("pi labels and a polar grid", () => {
    expect(plane(null, { labels: "pi", x: [-4, 4] })).toContain("π</text>");
    expect(plane(null, { grid: "polar" })).toContain("<ellipse");
  });

  test("Plane holds its height before it has measured", () => {
    const html = renderToStaticMarkup(<Plane2d.Plane height={240} />);
    expect(html).toBe('<div class="o-plane" style="height:240px"></div>');
    expect(renderToStaticMarkup(<Plane2d.Plane grid="none" />)).toContain('class="o-plane o-plane-bare" style="height:320px"');
  });

  test("every shape draws in token colours", () => {
    const html = plane(
      <>
        <Plane2d.Plot y={(x) => x * x} />
        <Plane2d.Plot r={() => 2} color="chart-2" />
        <Plane2d.Plot xy={(t) => [Math.cos(t), Math.sin(2 * t)]} domain={[0, 6]} color="chart-3" />
        <Plane2d.Point x={1} y={1} color="danger" />
        <Plane2d.MovablePoint point={[1, 2]} onMove={() => {}} />
        <Plane2d.Vector tip={[2, 1]} color="chart-4" />
        <Plane2d.Line point={[0, 0]} slope={2} color="muted" style="dashed" />
        <Plane2d.Segment point1={[0, 0]} point2={[1, 3]} color="success" />
        <Plane2d.Circle center={[0, 0]} radius={1} color="chart-5" />
        <Plane2d.Polygon points={[[0, 0], [1, 0], [0, 1]]} color="warning" />
        <Plane2d.Arc center={[0, 0]} radius={2} from={0} to={Math.PI} color="foreground" />
      </>,
    );
    for (const token of ["chart-1", "chart-2", "chart-3", "destructive", "brand", "chart-4", "muted-foreground", "success", "chart-5", "warning", "foreground"])
      expect(html).toContain(`var(--color-${token})`);
    expect(html).not.toMatch(/#[0-9a-f]{3,8}\b|rgb\(/i);
    // The polar curve went round: points on both sides of the origin.
    expect(html.match(/<path d="M /g)?.length).toBeGreaterThanOrEqual(3);
  });

  test("a colour that isn't a theme name is an error, not a colour", () => {
    expect(() => plane(<Plane2d.Point x={0} y={0} color={"#ff0000" as never} />)).toThrow(/isn't a theme colour; use one of foreground, muted/);
    expect(() => plane(<Plane2d.Plot y={(x) => x} color={"red" as never} />)).toThrow(/"red" isn't a theme colour/);
  });

  test("an arc is a path of quarter turns at most, counterclockwise", () => {
    const html = plane(<Plane2d.Arc center={[0, 0]} radius={1} from={0} to={Math.PI * 1.5} />);
    const d = html.match(/<path d="(M [^"]+)" fill="none"/)?.[1] ?? "";
    expect(d.match(/ A /g)?.length).toBe(3);
    expect(d).toContain(" 0 0 0 ");
  });

  test("a right angle is a square corner; another angle a wedge with its label", () => {
    const right = plane(<Plane2d.Angle vertex={[0, 0]} from={[1, 0]} to={[0, 2]} />);
    expect(right).toMatch(/class="o-plane-angle"><path d="M [^"]* L [^"]* L [^"]* L [^"]* Z"/);
    const acute = plane(<Plane2d.Angle vertex={[0, 0]} from={[1, 0]} to={[1, 1]} label="\\theta" color="chart-1" />);
    expect(acute).toMatch(/class="o-plane-angle"><path d="M [^"]* L [^"]* A /);
    expect(acute).toContain('class="o-plane-label"');
    expect(acute).toContain("\\theta");
  });

  test("a label is maths in a foreignObject, beside its point when attached", () => {
    const html = plane(<Plane2d.Label at={[0, 0]} tex="x^2" attach="ne" />);
    expect(html).toMatch(/<foreignObject x="8" y="-248" width="640" height="240" pointer-events="none" class="o-plane-label">/);
    expect(html).toContain("justify-content:flex-start;align-items:flex-end");
    // In a test the engine never loads: the TeX stays, muted.
    expect(html).toContain('<span class="o-tex o-tex-pending">x^2</span>');
  });
});

describe("the katex shim", () => {
  const element = (): Target & { isConnected: boolean } => ({ innerHTML: "", textContent: "", className: "", isConnected: true });
  const renderer = { renderTex: (tex: string, display: boolean) => `<b>${display ? "D" : "I"}:${tex}</b>` };

  test("draws at once when the engine is in", () => {
    const shim = katexShim(() => Promise.resolve(), () => renderer);
    const el = element();
    shim.render("x^2", el, { displayMode: true });
    expect(el.innerHTML).toBe("<b>D:x^2</b>");
    expect(el.className).toBe("o-tex");
    expect(shim.renderToString("y")).toBe("<b>I:y</b>");
  });

  test("shows the TeX until the engine loads, then the last TeX asked", async () => {
    let ready: typeof renderer | null = null;
    let resolve = () => {};
    const loaded = new Promise<void>((r) => (resolve = r));
    let loads = 0;
    const shim = katexShim(() => (loads++, loaded), () => ready);
    const el = element();
    const gone = element();
    shim.render("a", el);
    shim.render("b", el);
    shim.render("c", gone);
    gone.isConnected = false;
    expect(el.textContent).toBe("b");
    expect(el.className).toBe("o-tex o-tex-pending");
    expect(shim.renderToString("<x>")).toBe("&lt;x&gt;");
    ready = renderer;
    resolve();
    await loaded;
    await Promise.resolve();
    expect(el.innerHTML).toBe("<b>I:b</b>");
    expect(el.className).toBe("o-tex");
    // An element no longer on the page is left alone.
    expect(gone.textContent).toBe("c");
    expect(loads).toBe(2);
  });

  test("a failed load leaves the TeX showing", async () => {
    const shim = katexShim(() => Promise.resolve(), () => null);
    const el = element();
    shim.render("z", el);
    await Promise.resolve();
    await Promise.resolve();
    expect(el.textContent).toBe("z");
  });
});

describe("oculus/math", () => {
  test("a formula is a function of its variables, with its TeX", () => {
    const f = Maths.formula("a x^2 + sin(pi x)", ["x", "a"]);
    if (!f.ok) throw new Error(f.error);
    expect(f.fn(2, 3)).toBeCloseTo(12, 9);
    expect(f.tex).toContain("\\sin");
    const g = Maths.formula("sqrt(x)");
    if (!g.ok) throw new Error(g.error);
    expect(g.fn(9)).toBe(3);
    // Complex, so not a point on a real plot.
    expect(g.fn(-1)).toBeNaN();
  });

  test("a bad formula is a message, never a throw", () => {
    expect(Maths.formula("")).toEqual({ ok: false, error: "Type a formula" });
    const syntax = Maths.formula("sin(x");
    expect(syntax.ok).toBe(false);
    expect(!syntax.ok && syntax.error).toMatch(/^Couldn't read "sin\(x": /);
    expect(Maths.formula("x + y")).toEqual({ ok: false, error: `"y" isn't defined (the variables are x)` });
    expect(Maths.formula("gamma(x)")).toEqual({ ok: false, error: `"gamma" isn't a function here` });
    // The parser's own functions aren't maths a formula may call.
    expect(Maths.formula("evaluate(x)").ok).toBe(false);
  });

  test("a derivative evaluates as a formula", () => {
    const d = Maths.derivative("tan(x) * asin(x)", "x").toString();
    const f = Maths.formula(d);
    if (!f.ok) throw new Error(f.error);
    expect(f.fn(0.3)).toBeCloseTo(1 / Math.cos(0.3) ** 2 * Math.asin(0.3) + Math.tan(0.3) / Math.sqrt(1 - 0.09), 9);
  });

  test("the subset evaluates, simplifies, and does complex numbers and matrices", () => {
    expect(Maths.evaluate("2 + 3 * sin(pi / 2)")).toBe(5);
    expect(Maths.simplify("2x + 3x").toString()).toBe("5 * x");
    expect(Maths.evaluate("sqrt(-4)").toString()).toBe("2i");
    expect(Maths.det([[1, 2], [3, 4]])).toBeCloseTo(-2, 12);
    expect(Maths.eigs([[2, 0], [0, 3]]).values).toEqual([2, 3]);
    expect(Maths.compile("x + 1").evaluate({ x: 2 })).toBe(3);
  });
});

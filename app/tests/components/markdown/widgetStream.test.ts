import { describe, expect, test } from "bun:test";
import { closePrefix } from "@/components/markdown/widget/stream/repair";
import { moduleCode, previewSource } from "@/components/markdown/widget/stream/preview";
import { MAX_WAIT_MS, PAUSE_MS, PreviewScheduler, type Clock } from "@/components/markdown/widget/stream/schedule";
import { STILL_WRITING, streamGate } from "@/components/markdown/widget/stream/gate";
import type { WidgetRequest } from "@/components/markdown/widget/messages";
import { STILL_WRITING as FRAME_STILL_WRITING } from "../../../widget-kit/protocol";

// Bun's own TSX parser stands in for oxc here: both refuse what isn't TSX.
const tsx = new Bun.Transpiler({ loader: "tsx" });
const parses = (source: string) => {
  try {
    tsx.transformSync(source);
    return true;
  } catch {
    return false;
  }
};

describe("closePrefix", () => {
  // The spike's cases (data/evals/widget-stream-spike/repair.test.ts).
  const cases: Record<string, string> = {
    jsx: "export default function W(){ return (<div><span>Hi",
    nestedAttribute: 'export default function W(){ return (<div title={<span>{"title"}</span>}><b>Hi',
    expression: "export default function W(){return (<div>{[1,2].map(x=>(<span>{x}</span>))}",
    string: 'export default function W(){const x="unfinished',
    comment: "export default function W(){ /* unfinished",
    fragment: "export default function W(){return (<><div>hello</div>",
    generic: "export default function W(){ const x = useState<number>(0); return (<div>Hi",
    regex: "export default function W(){const x=/[)}]/; return (<div>Hi",
    lineComment: "export default function W(){ return <div/> // trailing",
    selfClosing: 'export default function W(){ return (<div><Slider label="a" value={1} /',
  };
  for (const [name, source] of Object.entries(cases)) {
    test(`${name} closes into TSX that parses`, () => {
      const out = closePrefix(source);
      expect(out).not.toBeNull();
      expect(out!.startsWith(source)).toBe(true);
      expect(parses(out!)).toBe(true);
    });
  }

  test("an expression cut mid-way is not invented", () => {
    const source = "export default function W(){const x=`hello ${1+";
    const out = closePrefix(source)!;
    expect(out.startsWith(source)).toBe(true);
    expect(parses(out)).toBe(false);
  });

  test("a whole program comes back as it was", () => {
    const s = "export default function W(){ return <div/>; }";
    expect(closePrefix(s)!.trim()).toBe(s);
  });

  test("closing JSX and brackets interleave in the order opened", () => {
    expect(closePrefix("const a = [f(<b>{x")).toBe("const a = [f(<b>{x\n}</b>)]");
  });

  test("a cut inside a loop's body is refused, a finished loop is not", () => {
    expect(closePrefix("export default function W(){ while (true) {\n  if (done) ")).toBeNull();
    expect(closePrefix("export default function W(){ for (let i = 0; i < n; i++) {\n  x.push(i)")).toBeNull();
    expect(closePrefix("export default function W(){ do {\n  step()")).toBeNull();
    const done = closePrefix("export default function W(){ while (a) { a = a.next; }\n  if (ok) { go()");
    expect(done).not.toBeNull();
    expect(parses(done!)).toBe(true);
    // A callback isn't a loop: its body may be cut.
    expect(closePrefix("export default function W(){ xs.forEach((x) => {\n  f(x)")).not.toBeNull();
  });
});

describe("previewSource", () => {
  test("tries only whole lines, and only once there is a default export", () => {
    expect(previewSource("// title: A\nimport { Widget } from \"oculus/ui\";\n")).toBeNull();
    expect(previewSource("// title: A\nexport default function W() {")).toBeNull();
    const out = previewSource("// title: A\nexport default function W() {\n  return <Widget><Row>{valu");
    expect(out).toBe("// title: A\nexport default function W() {\n}");
    expect(previewSource("export default function W() {\n  return <Widget>\n    <Row>\n")).toBe(
      "export default function W() {\n  return <Widget>\n    <Row>\n</Row></Widget>}",
    );
  });
});

describe("moduleCode", () => {
  const runtime = "http://127.0.0.1:1/widget-kit/runtime.js";
  test("a preview scopes its timers and stands in for parts not written yet", () => {
    const code = moduleCode({ js: "const __oculusWidget = W;", pending: ["Chart", "bad name"], id: 3, final: false, runtime });
    expect(code).toContain(`previewScope as __oculusScope } from "${runtime}"`);
    expect(code).toContain("const { setTimeout, clearTimeout, setInterval, clearInterval, requestAnimationFrame, cancelAnimationFrame } = __oculusScope(3);");
    expect(code).toContain('const Chart = __oculusPending("Chart");');
    expect(code).not.toContain("bad name =");
    expect(code.trimEnd().endsWith("__oculusUpdate(__oculusWidget, 3);")).toBe(true);
    expect(parses(code)).toBe(true);
  });
  test("the final program is the module as transpiled", () => {
    const code = moduleCode({ js: "const __oculusWidget = W;", pending: [], id: 4, final: true, runtime });
    expect(code).not.toContain("__oculusScope");
    expect(code).not.toContain("__oculusPending");
    // The stream ends before the widget's own top level runs, so its throws are reported.
    expect(code.indexOf("__oculusCommit(4);")).toBeLessThan(code.indexOf("const __oculusWidget = W;"));
    expect(code).toContain("__oculusUpdate(__oculusWidget, 4);");
    expect(parses(code)).toBe(true);
  });
});

/** A clock the test moves by hand. */
function fakeClock() {
  let now = 0;
  let timers: { at: number; fn: () => void; id: number }[] = [];
  let next = 0;
  const clock: Clock = {
    now: () => now,
    set: (fn, ms) => {
      const id = ++next;
      timers.push({ at: now + ms, fn, id });
      return id;
    },
    clear: (id) => {
      timers = timers.filter((t) => t.id !== id);
    },
  };
  const advance = async (ms: number) => {
    const end = now + ms;
    for (;;) {
      const due = timers.filter((t) => t.at <= end).sort((a, b) => a.at - b.at)[0];
      if (!due) break;
      timers = timers.filter((t) => t !== due);
      now = due.at;
      due.fn();
      await Promise.resolve();
      await Promise.resolve();
    }
    now = end;
  };
  return { clock, advance };
}

describe("PreviewScheduler", () => {
  const PROGRAM = "export default function W() {\n  return <div>\n    <b>one</b>\n    <b>two</b>\n    <b>three</b>\n";
  const prefix = (lines: number) => PROGRAM.split("\n").slice(0, lines).join("\n") + "\n";

  test("transpiles at a pause, once per repaired prefix, one at a time", async () => {
    const { clock, advance } = fakeClock();
    let source = prefix(2);
    const tried: string[] = [];
    let release: () => void = () => {};
    const s = new PreviewScheduler(
      () => source,
      (c) => {
        tried.push(c);
        return new Promise<void>((r) => (release = r));
      },
      clock,
    );
    s.poke();
    await advance(0);
    expect(tried.length).toBe(1); // the first try doesn't wait
    source = prefix(3);
    s.poke();
    await advance(PAUSE_MS * 3);
    expect(tried.length).toBe(1); // one in flight
    release();
    await advance(0);
    await advance(PAUSE_MS);
    expect(tried.length).toBe(2);
    expect(tried[1]).toContain("<b>one</b>");
    release();
    s.poke(); // nothing new
    await advance(PAUSE_MS);
    expect(tried.length).toBe(2);
    s.dispose();
  });

  test("a stream that never pauses still previews every MAX_WAIT_MS", async () => {
    const { clock, advance } = fakeClock();
    let lines = 2;
    const tried: number[] = [];
    let at = 0;
    const s = new PreviewScheduler(() => prefix(Math.min(lines, 6)), async () => void tried.push(at), clock);
    for (at = 0; at <= 2500; at += 48) {
      if (at % 480 === 0) lines++;
      s.poke();
      await advance(48);
    }
    s.dispose();
    // A delta every 48 ms is never a pause; the tries are paced by the cap.
    for (let i = 1; i < tried.length; i++) expect(tried[i] - tried[i - 1]).toBeGreaterThanOrEqual(MAX_WAIT_MS - 48);
    expect(tried.length).toBeGreaterThanOrEqual(2);
  });
});

describe("the streaming gate", () => {
  const ask: WidgetRequest = { type: "oculus-widget", kind: "ask", id: "r1", prompt: "p", schema: {} };
  const sendReq: WidgetRequest = { type: "oculus-widget", kind: "send", id: "r2", text: "hi" };

  test("while streaming, requests are refused and state is not saved", () => {
    const handled: string[] = [];
    const saved: unknown[] = [];
    const g = streamGate(true, (req) => handled.push(req.id), (s) => saved.push(s));
    const replies: unknown[] = [];
    g.onRequest!(ask, (r) => replies.push(r));
    g.onRequest!(sendReq, (r) => replies.push(r));
    g.onState({ a: 1 });
    expect(handled).toEqual([]);
    expect(saved).toEqual([]);
    expect(replies).toEqual([{ error: STILL_WRITING }, { error: STILL_WRITING }]);
  });

  test("once committed, both go where they always do", () => {
    const onRequest = () => {};
    const onState = () => {};
    const g = streamGate(false, onRequest, onState);
    expect(g.onRequest).toBe(onRequest);
    expect(g.onState).toBe(onState);
  });

  test("the frame and the parent give the same reason", () => {
    expect(STILL_WRITING).toBe(FRAME_STILL_WRITING);
  });
});

import { afterAll, describe, expect, spyOn, test } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
const globals = globalThis as { window?: unknown; document?: unknown };
const added = (["window", "document"] as const).filter(k => !(k in globals));
for (const k of added) globals[k] = Object.assign(new EventTarget(), { documentElement: { style: {} } });
afterAll(() => { for (const k of added) delete globals[k]; });
const { Renderer } = await import("@openuidev/react-lang");
const { library } = await import("@/components/markdown/openui/library");
const { parseProgram, streamVerdict } = await import("@/components/markdown/openui/parse");
const { StreamingPending } = await import("@/components/markdown/openui/Streaming");
const maths = await import("@/lib/maths/useMathsReady");
const { QueryStatus } = await import("@/components/markdown/openui/query/QueryStatus");

const CLT = `root = Stack([lesson])
$n = 10
lesson = Explorer("Averages of skewed samples", [Slider($n, "Sample size", 1, 100, 1), Chart(bins, "Sample mean", "Trials", "bar"), Value("Mean", @Mean(means)), Value("Spread", @StdDev(means))])
means = @Means(@Exponential(), {rate:1}, $n, 400, 42)
bins = @Bins(means, 20)`;
const QUERY = `root = Stack([Text("Your tasks"), summary])
$limit = 20
rows = Query("tasks", {limit:$limit}, [])
summary = Explorer("Live tasks", [Slider($limit, "Rows", 1, 100, 1), Value("Tasks", @Count(rows))])`;
const REVEAL = `root = Stack([Question("What changes as sample size grows?", [Option("The mean grows"), Option("The spread shrinks")], "The standard error scales as one over the square root of n.")])`;

describe("simulation, reveal and Query programs", () => {
  for (const [name, source] of Object.entries({ CLT, QUERY, REVEAL })) test(`${name}: every streaming prefix renders and the final program validates`, () => {
    const ready = spyOn(maths, "useMathsReady").mockReturnValue(true);
    try {
      for (let end = 0; end <= source.length; end++) {
        const prefix = source.slice(0, end);
        expect(() => renderToStaticMarkup(<StreamingPending.Provider value={streamVerdict(prefix).pending}>
          <Renderer response={prefix} library={library} isStreaming publishObservability={false}><QueryStatus /><Renderer.Content /></Renderer>
        </StreamingPending.Provider>)).not.toThrow();
      }
      const verdict = parseProgram(source);
      if (!verdict.ok) throw new Error(verdict.errors.join("; "));
    } finally { ready.mockRestore(); }
  });
  test("CLT bins render bars with Chart, while the source binds Means to a Slider", () => {
    const ready = spyOn(maths, "useMathsReady").mockReturnValue(true);
    try {
      // Server rendering has no initialization effect; resolve the declared initial
      // n for this render. The streaming test above checks the bound program.
      const html = renderToStaticMarkup(<Renderer response={CLT.replace('}, $n,', '}, 10,')} library={library} publishObservability={false}><Renderer.Content /></Renderer>);
      expect(html).toContain('role="slider"');
      expect(html).toContain('role="img"');
      expect(html.match(/<rect /g)).toHaveLength(20);
      expect(html).toContain("Sample mean");
      expect(html).not.toContain("Not enough points");
    } finally { ready.mockRestore(); }
  });
  test("standalone Question reveals without scoring or requiring correctness flags", () => {
    const ready = spyOn(maths, "useMathsReady").mockReturnValue(true);
    try {
      const html = renderToStaticMarkup(<Renderer response={REVEAL} library={library} publishObservability={false}><Renderer.Content /></Renderer>);
      expect(html).toContain("Check your intuition");
      expect(html).toContain("The spread shrinks");
      expect(html).not.toContain("The standard error");
      expect(html).not.toContain("See score");
      expect(html).not.toContain("Save to Revision");
    } finally { ready.mockRestore(); }
  });
  test("Data notes expose Query tool names and limits in the generated prompt", () => {
    const prompt = library.prompt();
    expect(prompt).toContain("Live read-only rows:");
    expect(prompt).toContain('"revision due"');
    expect(prompt).toContain("capped at 400");
  });
  test("read-only fences refuse Mutation", () => {
    const result = parseProgram('root = Stack([Text("No writes")])\nrows = Mutation("tasks", {}, [])');
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.errors.join()).toContain("Mutation is not supported");
  });
});

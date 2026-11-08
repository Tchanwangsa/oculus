import { afterAll, expect, spyOn, test } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";

const globals = globalThis as { window?: unknown; document?: unknown };
const added = (["window", "document"] as const).filter((key) => !(key in globals));
for (const key of added) globals[key] = Object.assign(new EventTarget(), { documentElement: { style: {} } });
afterAll(() => { for (const key of added) delete globals[key]; });
const { TextDiff } = await import("@/components/markdown/openui/proposals/Fields");
const maths = await import("@/lib/maths/useMathsReady");

test("markdown changes preserve block structure, formatting and maths on both sides", () => {
  const before = "## Original\n\n**Read this**\n\n| Topic | Time |\n| --- | --- |\n| Logic | 30 |\n\n$$\nx^2\n$$";
  const after = "## Revised\n\n- **Study** logic\n- Review proofs\n\n```js\nconst hours = 2;\n```\n\n$$\nx^3\n$$";
  const ready = spyOn(maths, "useMathsReady").mockReturnValue(true);
  let html: string;
  try { html = renderToStaticMarkup(<TextDiff before={before} after={after} markdown />); }
  finally { ready.mockRestore(); }
  expect(html).toContain("Removed");
  expect(html).toContain("Added");
  expect(html).toContain("<h2");
  expect(html).toContain("<table");
  expect(html).toContain("<strong>Read this</strong>");
  expect(html).toContain("<strong>Study</strong>");
  expect(html).toContain("<ul");
  expect(html).toContain("<pre");
  expect(html).toContain("katex");
  expect(html).not.toContain("**Read this**");
  expect(html).not.toContain("$$");
});

test("code changes keep their literal source instead of interpreting markdown", () => {
  const html = renderToStaticMarkup(<TextDiff before="const x = '**before**';" after="const x = '**after**';" />);
  expect(html).toContain("**before**");
  expect(html).toContain("**after**");
  expect(html).not.toContain("<strong>");
});

import { describe, expect, test } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";

// The bubble's import graph reaches modules that listen on `window` and
// `document` at load; they get inert stand-ins for the import, then the
// globals go back so no other test sees a half DOM.
const globals = globalThis as { window?: unknown; document?: unknown };
const before = { window: globals.window, document: globals.document };
const inert = { addEventListener: () => {}, removeEventListener: () => {} };
globals.window ??= inert;
globals.document ??= { ...inert, documentElement: { style: {} } };
const { Pending, PendingList } = await import("@/components/harness/timeline/Pending");
const { TooltipProvider } = await import("@/components/ui/tooltip");
Object.assign(globals, before);

const actions = {
  editQueued: async () => {},
  unqueue: async () => {},
  reorder: async () => {},
};

function render(queued: { id: string; text: string; held: boolean }[]) {
  return renderToStaticMarkup(
    <TooltipProvider>
      <PendingList threadId={1} queued={queued} actions={actions} />
    </TooltipProvider>,
  );
}

describe("Pending", () => {
  test("a held message says Held in words; a waiting one says Waiting", () => {
    const html = render([
      { id: "1", text: "first", held: true },
      { id: "2", text: "second", held: false },
    ]);
    expect(html.indexOf("Held")).toBeLessThan(html.indexOf("first"));
    expect(html.indexOf("Waiting")).toBeLessThan(html.indexOf("second"));
  });

  test("more than one: each can move, but not past the ends", () => {
    const html = render([
      { id: "1", text: "first", held: true },
      { id: "2", text: "second", held: true },
    ]);
    expect(html.match(/aria-label="Move earlier"/g)?.length).toBe(2);
    expect(html.match(/aria-label="Move later"/g)?.length).toBe(2);
    expect(html).toMatch(/aria-label="Move earlier" disabled=""/);
    expect(html).toMatch(/aria-label="Move later" disabled=""/);
  });

  test("a single message has no order to change", () => {
    const html = render([{ id: "1", text: "only", held: true }]);
    expect(html).not.toContain("Move earlier");
    expect(html).toContain("Held");
  });

  test("nothing waiting draws nothing", () => {
    // Server rendering reads the store's initial state: no thread has a queue.
    expect(renderToStaticMarkup(<Pending threadId={1} actions={actions} />)).toBe("");
  });
});

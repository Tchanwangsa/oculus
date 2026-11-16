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
  steerQueued: async () => {},
};

function render(queued: { id: string; text: string; held: boolean }[], steer: "native" | "restart" | null = null) {
  return renderToStaticMarkup(
    <TooltipProvider>
      <PendingList threadId={1} queued={queued} actions={actions} steer={steer} />
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

  test("Send now on each message the running turn can take, its tooltip saying where it goes", () => {
    const html = render([
      { id: "1", text: "first", held: false },
      { id: "2", text: "second", held: false },
    ], "native");
    expect(html.match(/>Send now</g)?.length).toBe(2);
    expect(html.match(/aria-description="Send into the current turn"/g)?.length).toBe(2);
    // It sits on the message's own line, above its bubble.
    expect(html.indexOf("Send now")).toBeLessThan(html.indexOf("first"));
  });

  test("on a restart-class bridge Send now says it stops and restarts the turn", () => {
    const html = render([{ id: "1", text: "first", held: false }], "restart");
    expect(html).toContain('aria-description="Stops and restarts the turn"');
  });

  test("no Send now where nothing can be steered, nor on a slash command", () => {
    expect(render([{ id: "1", text: "first", held: true }], null)).not.toContain("Send now");
    const html = render([
      { id: "1", text: "/compact", held: false },
      { id: "2", text: "plain", held: false },
    ], "native");
    expect(html.match(/>Send now</g)?.length).toBe(1);
    expect(html.indexOf("Send now")).toBeGreaterThan(html.indexOf("/compact"));
  });

  test("nothing waiting draws nothing", () => {
    // Server rendering reads the store's initial state: no thread has a queue.
    expect(renderToStaticMarkup(<Pending threadId={1} actions={actions} />)).toBe("");
  });
});

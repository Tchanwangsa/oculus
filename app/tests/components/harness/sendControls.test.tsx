import { describe, expect, test } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import { SendControls } from "@/components/harness/composer/SendControls";
import { TooltipProvider } from "@/components/ui/tooltip";
import { RESTART_HINT, runControls, type RunControlInputs } from "@/lib/harness";

const noop = () => {};
const idle: RunControlInputs = {
  running: false,
  liveRun: null,
  stopSent: false,
  held: false,
  hasDraft: false,
  ready: false,
  writing: false,
};
const render = (i: RunControlInputs) =>
  renderToStaticMarkup(
    <TooltipProvider>
      <SendControls {...runControls(i)} onStop={noop} onSend={noop} onResume={noop} />
    </TooltipProvider>,
  );

describe("SendControls", () => {
  test("running: an icon Stop labelled Stop generation", () => {
    const html = render({ ...idle, running: true, liveRun: { id: 3, stopping: false } });
    expect(html).toContain('aria-label="Stop generation"');
    expect(html).not.toContain("Stopping");
  });

  test("stopping: the words Stopping, disabled, with a brand spinner", () => {
    const html = render({ ...idle, running: true, liveRun: { id: 3, stopping: true } });
    expect(html).toMatch(/<button[^>]*disabled=""[^>]*>.*Stopping<\/button>/);
    expect(html).toContain("animate-spin text-brand");
    expect(html).not.toContain("Stop generation");
  });

  test("a held queue and an empty box: a Resume thread pill", () => {
    const html = render({ ...idle, held: true });
    expect(html).toMatch(/<button[^>]*rounded-full[^>]*>.*Resume thread<\/button>/);
    expect(html).not.toContain("aria-label=\"Send");
  });

  test("a held queue and a draft: the send arrow says where the draft goes", () => {
    const html = render({ ...idle, held: true, hasDraft: true, ready: true });
    expect(html).toContain('aria-label="Send after the waiting messages"');
    expect(html).not.toContain("Resume");
  });

  test("idle: Send message; queueing: Queue message", () => {
    expect(render({ ...idle, hasDraft: true, ready: true })).toContain('aria-label="Send message"');
    expect(render({ ...idle, running: true, hasDraft: true, ready: true })).toContain('aria-label="Queue message"');
  });

  test("steering: Steer message with the arrow; queueing: Queue message with a list-plus, so the icon flips with ⌘", () => {
    const working = { ...idle, running: true, liveRun: { id: 3, stopping: false }, hasDraft: true, ready: true };
    const steer = render({ ...working, action: { kind: "steer", restart: false } });
    const queue = render({ ...working, action: { kind: "queue" } });
    expect(steer).toContain('aria-label="Steer message"');
    expect(steer).toContain('aria-description="Send into the current turn"');
    expect(queue).toContain('aria-label="Queue message"');
    const icon = (html: string) => html.match(/<svg[^>]*>([\s\S]*?)<\/svg>/g)?.at(-1);
    expect(icon(steer)).not.toEqual(icon(queue));
  });

  test("a restart-class steer says it stops and restarts the turn", () => {
    const html = render({ ...idle, running: true, liveRun: { id: 3, stopping: false }, hasDraft: true, ready: true, action: { kind: "steer", restart: true } });
    expect(html).toContain(`aria-description="${RESTART_HINT}"`);
  });
});

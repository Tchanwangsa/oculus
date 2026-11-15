import { describe, expect, test } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import { SendControls } from "@/components/harness/composer/SendControls";
import { runControls, type RunControlInputs } from "@/lib/harness";

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
  renderToStaticMarkup(<SendControls {...runControls(i)} onStop={noop} onSend={noop} onResume={noop} />);

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
});

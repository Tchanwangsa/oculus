import { expect, test } from "bun:test";
import { runInNewContext } from "node:vm";
import {
  DEFAULT_HEIGHT,
  REPORT_HEIGHT,
  initialPageHeight,
  nextPageHeight,
  readPageHeight,
  type PageHeightReport,
} from "@/components/markdown/embed/pagePrepare";

// Execute the injected script without a browser, controlling the layout and
// observer notifications to exercise its messages through the real height rule.
function reporter() {
  const events = new Map<string, (() => void)[]>();
  const messages: PageHeightReport[] = [];
  const source = {} as Window;
  let contentHeight = DEFAULT_HEIGHT;
  let viewportSized = false;
  let mutation: () => void = () => {};
  let resize: () => void = () => {};
  let fonts: () => void = () => {};
  const root = { getBoundingClientRect: () => ({ height: measure() }) };
  const body = { get scrollHeight() { return measure(); } };
  const scope = {
    innerHeight: DEFAULT_HEIGHT,
    innerWidth: 800,
    document: {
      documentElement: root,
      body,
      fonts: { addEventListener(_type: string, callback: () => void) { fonts = callback; } },
    },
    addEventListener(type: string, callback: () => void) {
      events.set(type, [...(events.get(type) ?? []), callback]);
    },
    parent: {
      postMessage(data: unknown) {
        const report = readPageHeight({ source, data }, source);
        if (!report) throw new Error("Reporter emitted an invalid message");
        messages.push(report);
      },
    },
    ResizeObserver: class {
      constructor(callback: () => void) { resize = callback; }
      observe() {}
    },
    MutationObserver: class {
      constructor(callback: () => void) { mutation = callback; }
      observe(_root: unknown, options: MutationObserverInit) {
        expect(options).toMatchObject({ subtree: true, childList: true, attributes: true, characterData: true });
      }
    },
  };
  function measure() { return viewportSized ? scope.innerHeight + 16 : contentHeight; }
  function emit(type: string) { for (const listener of events.get(type) ?? []) listener(); }
  runInNewContext(REPORT_HEIGHT, scope);
  return {
    messages,
    emit,
    resize,
    mutation,
    fonts,
    setContent(height: number) { contentHeight = height; },
    useViewportHeight() { viewportSized = true; },
    setViewport(height: number, width = scope.innerWidth) {
      scope.innerHeight = height;
      scope.innerWidth = width;
      emit("resize");
      resize();
    },
  };
}

test("the injected reporter and height rule let equal-sized rows keep growing", () => {
  const frame = reporter();
  frame.emit("DOMContentLoaded");
  let state = initialPageHeight();
  function receive() {
    while (frame.messages.length) state = nextPageHeight(state, frame.messages.shift()!);
  }
  receive();
  for (let i = 1; i <= 20; i++) {
    const height = DEFAULT_HEIGHT + i * 32;
    frame.setContent(height);
    frame.mutation();
    frame.resize();
    receive();
    expect(state.height).toBe(height);
    expect(state.held).toBe(false);
    frame.setViewport(state.height);
    receive();
  }
});

test("the injected reporter stops viewport feedback and content changes release the hold", () => {
  const frame = reporter();
  frame.useViewportHeight();
  frame.emit("DOMContentLoaded");
  let state = initialPageHeight();
  let reports = 0;
  while (frame.messages.length) {
    if (++reports > 10) throw new Error("Viewport feedback did not stop");
    const height = state.height;
    state = nextPageHeight(state, frame.messages.shift()!);
    if (state.height !== height) frame.setViewport(state.height);
  }
  expect(state.held).toBe(true);
  expect(state.height).toBe(368);
  frame.mutation();
  const changed = nextPageHeight(state, frame.messages.shift()!);
  expect(changed.held).toBe(false);
  expect(changed.height).toBe(384);
});

test("the reporter emits viewport-only changes and content revisions at unchanged height", () => {
  const frame = reporter();
  frame.emit("DOMContentLoaded");
  const first = frame.messages.pop()!;
  frame.resize();
  expect(frame.messages).toHaveLength(0);
  frame.setViewport(400, 600);
  const reflow = frame.messages.pop()!;
  expect(reflow).toMatchObject({ height: DEFAULT_HEIGHT, viewportHeight: 400, viewportWidth: 600, revision: first.revision });
  let revision = first.revision;
  for (const changed of [frame.mutation, () => frame.emit("load"), () => frame.emit("error"), frame.fonts,
    () => frame.emit("input"), () => frame.emit("change"), () => frame.emit("animationend"), () => frame.emit("transitionend")]) {
    changed();
    const report = frame.messages.pop()!;
    expect(report.height).toBe(DEFAULT_HEIGHT);
    expect(report.revision).toBe(++revision);
  }
});

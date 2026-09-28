import { afterAll, beforeEach, describe, expect, test } from "bun:test";
import type { AppTab } from "../src/stores/tabStore";

// The store's playback dependency installs visibility/pagehide listeners.
// Supply only their event surfaces while loading the store outside a WebView.
const previousWindow = globalThis.window;
const previousDocument = globalThis.document;
Object.assign(globalThis, { window: new EventTarget(), document: new EventTarget() });
const { activePane, browserPageMayHide, useTabStore } = await import("../src/stores/tabStore");
const { openUrlInFocusedPane, registerTabRouter, unregisterTabRouter } = await import("../src/lib/tabRouters");
const { focusBrowserInput } = await import("../src/lib/browser");
Object.assign(globalThis, { window: previousWindow, document: previousDocument });

const initial = useTabStore.getState();
const tabs: AppTab[] = [
  { id: 1, path: "/calendar", canBack: false, canForward: false, focus: "main",
    split: { id: 2, path: "/browse/11", canBack: false, canForward: false } },
  { id: 3, path: "/browse/12", canBack: false, canForward: false, focus: "main", split: null },
];

beforeEach(() => useTabStore.setState({ tabs, activeId: 1, closed: [] }));
afterAll(() => useTabStore.setState(initial));

describe("native browser focus in split tabs", () => {
  test("a browser body click immediately routes shell actions to its pane", () => {
    expect(activePane()?.path).toBe("/calendar");
    useTabStore.getState().focusBrowserPane(11);
    expect(activePane()?.id).toBe(2);
    expect(activePane()?.path).toBe("/browse/11");
    useTabStore.getState().focusPane(1, "main");
    expect(activePane()?.path).toBe("/calendar");
  });

  test("background, removed and repeated focus events do not move the active tab", () => {
    const before = useTabStore.getState();
    useTabStore.getState().focusBrowserPane(12);
    useTabStore.getState().focusBrowserPane(99);
    expect(useTabStore.getState()).toBe(before);
    useTabStore.getState().focusBrowserPane(11);
    const focused = useTabStore.getState();
    useTabStore.getState().focusBrowserPane(11);
    expect(useTabStore.getState()).toBe(focused);
    useTabStore.setState({ tabs: [{ ...tabs[0], split: null }] });
    useTabStore.getState().focusBrowserPane(11);
    expect(activePane()?.id).toBe(1);
    expect(useTabStore.getState().activeId).toBe(1);
  });

  test("either native browser can regain focus without requesting native focus", () => {
    useTabStore.setState({ tabs: [{ ...tabs[0], path: "/browse/10" }] });
    useTabStore.getState().focusBrowserPane(11);
    expect(activePane()?.id).toBe(2);
    useTabStore.getState().focusBrowserPane(10);
    expect(activePane()?.id).toBe(1);
  });
});

describe("browser adoption into a split pane", () => {
  async function openWithSnapshot(duringOpen?: () => void) {
    const previous = globalThis.window;
    useTabStore.setState({
      tabs: [{ ...tabs[0], focus: "split", split: { ...tabs[0].split!, path: "/new" } }, tabs[1]],
      activeId: 1,
    });
    registerTabRouter(2, {
      navigate: (path: string) => useTabStore.getState().setPath(2, path, { canBack: true, canForward: false }),
    } as unknown as Parameters<typeof registerTabRouter>[1]);
    Object.assign(globalThis, { window: { __TAURI_INTERNALS__: {
      invoke: async (command: string) => {
        expect(command).toBe("browser_open_url");
        // The browser-state event arrives before its open IPC resolves, giving
        // the page a temporary foreground tab after an unrelated subject tab.
        useTabStore.setState((s) => ({
          tabs: [...s.tabs, { id: 4, path: "/browse/11", canBack: false, canForward: false, split: null, focus: "main" }],
          activeId: 4,
        }));
        duringOpen?.();
        return 11;
      },
    } } });
    try {
      await openUrlInFocusedPane("https://example.com");
    } finally {
      unregisterTabRouter(2);
      Object.assign(globalThis, { window: previous });
    }
  }

  test("closing the temporary browser tab restores its requesting split rather than its neighbour", async () => {
    await openWithSnapshot();
    expect(useTabStore.getState().activeId).toBe(1);
    expect(activePane()?.path).toBe("/browse/11");
    expect(useTabStore.getState().tabs.map((t) => t.id)).toEqual([1, 3]);
  });

  test("adoption preserves a deliberate switch made while the native page opens", async () => {
    await openWithSnapshot(() => useTabStore.getState().setActive(3));
    expect(useTabStore.getState().activeId).toBe(3);
    expect(useTabStore.getState().tabs[0].split?.path).toBe("/browse/11");
    expect(useTabStore.getState().tabs.map((t) => t.id)).toEqual([1, 3]);
  });

  test("closing the destination while opening leaves the browser in its own tab", async () => {
    await openWithSnapshot(() => useTabStore.getState().closeSplit(1));
    expect(useTabStore.getState().activeId).toBe(4);
    expect(activePane()?.path).toBe("/browse/11");
    expect(useTabStore.getState().tabs[0].split).toBeNull();
  });

  test("temporary page cleanup cannot hide the newly adopted visible browser", async () => {
    await openWithSnapshot();
    // Both the old strip page's unmount and an obsolete inactive effect can
    // run after the destination's place command; neither may hide its page.
    expect(browserPageMayHide(11)).toBe(false);
    expect(browserPageMayHide(11, 4)).toBe(false);
    // The destination still needs to hide itself under its own address menu.
    expect(browserPageMayHide(11, 2)).toBe(true);
    useTabStore.getState().setActive(3);
    expect(browserPageMayHide(11)).toBe(true);
  });

  test("the final owner leaving the screen permits native cleanup", () => {
    expect(browserPageMayHide(11)).toBe(false);
    useTabStore.getState().closeSplit(1);
    expect(browserPageMayHide(11)).toBe(true);
    expect(browserPageMayHide(99)).toBe(true);
  });
});

describe("native keyboard focus handoff", () => {
  async function handoff(change: "none" | "pending" | "frame" | "inactive") {
    const savedWindow = globalThis.window;
    const savedFrame = globalThis.requestAnimationFrame;
    let active = change !== "inactive";
    let release!: () => void;
    const nativeFocus = new Promise<void>((resolve) => { release = resolve; });
    const calls: string[] = [];
    let frame: FrameRequestCallback | undefined;
    Object.assign(globalThis, {
      window: { __TAURI_INTERNALS__: {
        invoke: (command: string) => { calls.push(command); return nativeFocus; },
      } },
      requestAnimationFrame: (callback: FrameRequestCallback) => { frame = callback; return 1; },
    });
    try {
      const done = focusBrowserInput(
        () => active,
        () => calls.push("reveal"),
        () => ({ focus: () => calls.push("focus"), select: () => calls.push("select") }),
      );
      expect(calls).toEqual(change === "inactive" ? [] : ["reveal"]);
      if (change === "frame") active = false;
      frame?.(0);
      await Promise.resolve();
      if (change === "pending") active = false;
      release();
      await done;
      return calls;
    } finally {
      Object.assign(globalThis, { window: savedWindow, requestAnimationFrame: savedFrame });
    }
  }

  test("primes the mounted input before native focus and restores its caret after the handoff", async () => {
    expect(await handoff("none")).toEqual(["reveal", "focus", "select", "browser_focus_main", "focus", "select"]);
  });

  test("does not reclaim DOM focus if the user leaves while the native command is pending", async () => {
    expect(await handoff("pending")).toEqual(["reveal", "focus", "select", "browser_focus_main"]);
  });

  test("rechecks the pane after the field mounts and ignores inactive shortcuts", async () => {
    expect(await handoff("frame")).toEqual(["reveal"]);
    expect(await handoff("inactive")).toEqual([]);
  });
});

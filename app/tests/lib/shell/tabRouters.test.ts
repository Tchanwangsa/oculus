import { afterAll, afterEach, beforeEach, describe, expect, spyOn, test } from "bun:test";
import { createMemoryRouter, type DataRouter } from "react-router-dom";
import { browser, browsePath } from "@/lib/browser";
import { pane } from "@/stores/shell/tabs/panes";


// Playback installs a pagehide listener when the shell is imported.
const oldWindow = Object.getOwnPropertyDescriptor(globalThis, "window");
const oldDocument = Object.getOwnPropertyDescriptor(globalThis, "document");
if (!oldDocument) Object.defineProperty(globalThis, "document", {
  configurable: true, value: { addEventListener() {} },
});
if (!oldWindow) Object.defineProperty(globalThis, "window", {
  configurable: true, value: { addEventListener() {}, setTimeout, clearTimeout },
});
const { dropTabRouter, openLink, openUrlInFocusedPane, routerFor } = await import("@/lib/shell/tabRouters");
const { sideFront, useTabStore } = await import("@/stores/shell/tabStore");
const originalState = useTabStore.getState();
afterAll(() => {
  useTabStore.setState(originalState);
  if (!oldWindow) Reflect.deleteProperty(globalThis, "window");
  if (!oldDocument) Reflect.deleteProperty(globalThis, "document");
});

const parentId = 1000;
const sideId = 1001;
const neighbourId = 1002;
const pageId = 41;
const url = "https://example.com";
let router: DataRouter;
let finishLazy: () => void;
let lazyStarted: Promise<void>;
let nativeOpen: ReturnType<typeof spyOn<typeof browser, "open">>;

beforeEach(() => {
  useTabStore.setState({
    tabs: [
      { ...pane(parentId, "/document"), focus: "side", side: {
        items: [{ ...pane(sideId, "/new"), viewed: 1 }], front: sideId, ratio: 0.5,
      } },
      { ...pane(neighbourId, "/new"), focus: "main", side: null },
    ], activeId: parentId, closed: [],
  });
  let markStarted: () => void;
  lazyStarted = new Promise<void>((resolve) => { markStarted = resolve; });
  const lazy = new Promise<void>((resolve) => { finishLazy = resolve; });
  router = routerFor(sideId, () => createMemoryRouter([
    { path: "/new", Component: () => null },
    { path: "/browse/:id", lazy: async () => {
      markStarted();
      await lazy;
      return { Component: () => null };
    } },
  ], { initialEntries: ["/new"] }));
  nativeOpen = spyOn(browser, "open");
});

afterEach(async () => {
  finishLazy();
  // Drain the lazy navigation before releasing its store subscription.
  for (let i = 0; i < 20 && router.state.navigation.state !== "idle"; i++)
    await new Promise((resolve) => setTimeout(resolve, 0));
  nativeOpen.mockRestore();
  for (const tab of useTabStore.getState().tabs) dropTabRouter(tab.id);
  dropTabRouter(sideId);
  router.dispose();
});

function snapshotBeforeReply() {
  nativeOpen.mockImplementation(async () => {
    // Rust broadcasts a new page before browser.open resolves; the shell
    // adopts an unclaimed page in a strip tab and activates it.
    useTabStore.getState().addTab(browsePath(pageId));
    return pageId;
  });
}

describe("opening a browser in the side panel", () => {
  test("claims the page while its first lazy route is still loading", async () => {
    nativeOpen.mockResolvedValue(pageId);
    await openUrlInFocusedPane(url);
    await lazyStarted;
    expect(router.state.location.pathname).toBe("/new");
    expect(router.state.navigation.location?.pathname).toBe(browsePath(pageId));
    const parent = useTabStore.getState().tabs.find((t) => t.id === parentId)!;
    expect(sideFront(parent)?.path).toBe(browsePath(pageId));
    expect(useTabStore.getState().activeId).toBe(parentId);
  });

  test("removing an early strip tab returns focus to its owning tab", async () => {
    snapshotBeforeReply();
    await openUrlInFocusedPane(url);
    await lazyStarted;
    const state = useTabStore.getState();
    expect(state.tabs.map((t) => t.id)).toEqual([parentId, neighbourId]);
    expect(state.activeId).toBe(parentId);
    expect(state.tabs[0].focus).toBe("side");
    expect(sideFront(state.tabs[0])?.path).toBe(browsePath(pageId));
  });

  test("cleanup preserves a different tab selected while the native page opens", async () => {
    nativeOpen.mockImplementation(async () => {
      useTabStore.getState().addTab(browsePath(pageId));
      useTabStore.getState().setActive(neighbourId);
      return pageId;
    });
    await openUrlInFocusedPane(url);
    expect(useTabStore.getState().activeId).toBe(neighbourId);
  });

  test("a plain web link keeps the source tab when the snapshot arrives first", async () => {
    snapshotBeforeReply();
    await openLink(url);
    const state = useTabStore.getState();
    expect(state.tabs.map((t) => t.id)).toEqual([parentId, neighbourId]);
    expect(state.activeId).toBe(parentId);
    expect(sideFront(state.tabs[0])?.path).toBe(browsePath(pageId));
  });

  test("an explicit new-tab link keeps its native strip tab", async () => {
    snapshotBeforeReply();
    await openLink(url, true);
    const state = useTabStore.getState();
    expect(state.tabs).toHaveLength(3);
    expect(state.tabs.find((t) => t.id === state.activeId)?.path).toBe(browsePath(pageId));
    expect(sideFront(state.tabs[0])?.path).toBe("/new");
  });
});

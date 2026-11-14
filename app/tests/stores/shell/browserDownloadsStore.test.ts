import { beforeEach, expect, test } from "bun:test";
import { EMPTY_PROMPT } from "@/lib/browser/downloads/prompt";
import { useBrowserDownloadsStore } from "@/stores/shell/browserDownloadsStore";
import { download } from "../../lib/browser/downloads/fixtures";

const store = useBrowserDownloadsStore;

beforeEach(() =>
  store.setState({
    downloads: [],
    seen: false,
    problem: false,
    prompt: EMPTY_PROMPT,
    view: { front: [], live: new Set() },
  }),
);

test("the button shows from the first download on, even once the list is cleared", () => {
  store.getState().apply({ downloads: [] }, 0, true);
  expect(store.getState().seen).toBe(false);
  store.getState().apply({ downloads: [download(1, { state: "failed", error: "x" })] }, 1);
  expect(store.getState().seen).toBe(true);
  store.getState().apply({ downloads: [] }, 2);
  expect(store.getState().seen).toBe(true);
});

test("the dialog follows the tabs in front: asks for the front tab, dismisses on a switch", () => {
  store.getState().setView({ front: [1], live: new Set([1, 2]) }, 0);
  store.getState().apply({ downloads: [download(1, { tab: 1 })] }, 10);
  expect(store.getState().prompt.open).toBe(1);
  store.getState().setView({ front: [2], live: new Set([1, 2]) }, 20);
  expect(store.getState().prompt.open).toBeNull();
  store.getState().ask(1);
  expect(store.getState().prompt.open).toBe(1);
  store.getState().dismiss();
  expect(store.getState().prompt.open).toBeNull();
  expect(store.getState().downloads).toHaveLength(1);
});

test("a download that fails or is refused marks the button until the list is opened", () => {
  store.getState().apply({ downloads: [download(1, { state: "failed", error: "x" })] }, 0, true);
  expect(store.getState().problem).toBe(false);
  store.getState().apply({ downloads: [download(1, { state: "failed", error: "x" }), download(2)] }, 10);
  expect(store.getState().problem).toBe(false);

  const failed = [download(1, { state: "failed", error: "x" }), download(2, { state: "failed", error: "x" })];
  store.getState().apply({ downloads: failed }, 20);
  expect(store.getState().problem).toBe(true);
  store.getState().acknowledge();
  expect(store.getState().problem).toBe(false);
  // The same list again (a progress re-send) is not a new problem.
  store.getState().apply({ downloads: failed }, 30);
  expect(store.getState().problem).toBe(false);

  const refused = download(3, { state: "refused", error: "a playlist" });
  store.getState().apply({ downloads: [...failed, refused] }, 40);
  expect(store.getState().problem).toBe(true);
});

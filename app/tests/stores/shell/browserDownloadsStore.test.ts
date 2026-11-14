import { beforeEach, expect, test } from "bun:test";
import { EMPTY_PROMPT } from "@/lib/browser/downloads/prompt";
import { useBrowserDownloadsStore } from "@/stores/shell/browserDownloadsStore";
import { download } from "../../lib/browser/downloads/fixtures";

const store = useBrowserDownloadsStore;

beforeEach(() =>
  store.setState({ downloads: [], seen: false, prompt: EMPTY_PROMPT, view: { front: [], live: new Set() } }),
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

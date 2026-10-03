import { beforeEach, describe, expect, test } from "bun:test";
import { useBrowserStore } from "../src/stores/browserStore";
import type { BrowserTab } from "../src/lib/browser";

const tab = (id: number): BrowserTab => ({
  id, url: `https://example.com/${id}`, title: `Page ${id}`, loading: false,
  can_back: false, can_forward: false, zoom: 1,
});

beforeEach(() => useBrowserStore.setState({ tabs: [], loaded: false, favicons: {} }));

describe("native browser snapshots", () => {
  test("publishes the first empty snapshot but skips repeated identical snapshots", () => {
    let updates = 0;
    const off = useBrowserStore.subscribe(() => updates++);
    useBrowserStore.getState().apply({ tabs: [] });
    expect(useBrowserStore.getState().loaded).toBe(true);
    useBrowserStore.getState().apply({ tabs: [] });
    expect(updates).toBe(1);
    off();
  });

  test("another page's load preserves unaffected pane objects, including across reordering", () => {
    const rows = [tab(1), tab(2)];
    useBrowserStore.getState().apply({ tabs: rows });
    const loading = { ...rows[1], loading: true };
    useBrowserStore.getState().apply({ tabs: [{ ...rows[0] }, loading] });
    expect(useBrowserStore.getState().tabs[0]).toBe(rows[0]);
    expect(useBrowserStore.getState().tabs[1]).toBe(loading);

    const state = useBrowserStore.getState();
    useBrowserStore.getState().apply({ tabs: [{ ...rows[0] }, { ...loading }] });
    expect(useBrowserStore.getState()).toBe(state);
    useBrowserStore.getState().apply({ tabs: [{ ...loading }, { ...rows[0] }] });
    expect(useBrowserStore.getState().tabs).toEqual([loading, rows[0]]);
    expect(useBrowserStore.getState().tabs[0]).toBe(loading);
    expect(useBrowserStore.getState().tabs[1]).toBe(rows[0]);
  });

  test("deleted and new pages are reflected without replacing surviving page objects", () => {
    const rows = [tab(1), tab(2)];
    useBrowserStore.getState().apply({ tabs: rows });
    const added = tab(3);
    useBrowserStore.getState().apply({ tabs: [{ ...rows[1] }, added] });
    expect(useBrowserStore.getState().tabs.map((row) => row.id)).toEqual([2, 3]);
    expect(useBrowserStore.getState().tabs[0]).toBe(rows[1]);
  });

  test("same favicon data does not publish and persisted icons cannot overwrite fresh icons", () => {
    let updates = 0;
    const off = useBrowserStore.subscribe(() => updates++);
    const store = useBrowserStore.getState();
    store.setFavicon("example.com", "fresh");
    store.setFavicon("example.com", "fresh");
    store.seedFavicons({ "example.com": "old" });
    expect(updates).toBe(1);
    store.seedFavicons({ "other.com": "saved" });
    expect(useBrowserStore.getState().favicons).toEqual({ "example.com": "fresh", "other.com": "saved" });
    expect(updates).toBe(2);
    off();
  });
});

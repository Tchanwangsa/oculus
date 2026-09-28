import { expect, test } from "bun:test";
import { SCRAPED_FILE_SAVED_EVENT } from "../src/lib/syncWrites";

test("new-file badges wait for committed metadata instead of raw scrape completion", async () => {
  const originalWindow = globalThis.window;
  const originalDocument = globalThis.document;
  const events = new EventTarget();
  // The store's file-access dependency registers the lecture visibility hook
  // at import time. Supply its event surface without loading a browser.
  Object.assign(globalThis, { window: events, document: new EventTarget() });
  const { useNewFilesStore, watchNewFiles } = await import("../src/stores/newFilesStore");
  const originalRefresh = useNewFilesStore.getState().refresh;
  let refreshes = 0;
  useNewFilesStore.setState({ refresh: async () => { refreshes++; } });
  const stop = watchNewFiles();
  try {
    expect(refreshes).toBe(1);
    // The scraper may finish while its metadata is still queued for SQLite.
    events.dispatchEvent(new Event("scrape-file"));
    events.dispatchEvent(new Event("scrape-complete"));
    await Bun.sleep(1600);
    expect(refreshes).toBe(1);
    events.dispatchEvent(new Event(SCRAPED_FILE_SAVED_EVENT));
    events.dispatchEvent(new Event(SCRAPED_FILE_SAVED_EVENT));
    await Bun.sleep(1600);
    expect(refreshes).toBe(2);
  } finally {
    stop();
    useNewFilesStore.setState({ refresh: originalRefresh });
    Object.assign(globalThis, { window: originalWindow, document: originalDocument });
  }
});

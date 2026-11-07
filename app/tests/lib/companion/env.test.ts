import { expect, test } from "bun:test";
import { browserShareEnv, type ShareLifecycleEvent, type SharePage } from "@/lib/companion/env";

test("browser visibility changes emit hidden and visible and remove their listener on cleanup", () => {
  const listeners = new Map<string, () => void>();
  const document = {
    visibilityState: "hidden",
    addEventListener: (type: string, listener: () => void) => { listeners.set(type, listener); },
    removeEventListener: (type: string, listener: () => void) => { if (listeners.get(type) === listener) listeners.delete(type); },
  };
  const page = {
    location: { origin: "http://localhost:43821" }, document,
    addEventListener() {}, removeEventListener() {},
  } as unknown as SharePage;
  const events: ShareLifecycleEvent[] = [];
  const stop = browserShareEnv(page).lifecycle(event => events.push(event));
  listeners.get("visibilitychange")!();
  document.visibilityState = "visible";
  listeners.get("visibilitychange")!();
  expect(events).toEqual(["hidden", "visible"]);
  stop();
  expect(listeners.size).toBe(0);
});

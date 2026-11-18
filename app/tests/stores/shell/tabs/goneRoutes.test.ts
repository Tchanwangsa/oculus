import { afterEach, describe, expect, test } from "bun:test";
import { isGoneRoute } from "@/lib/shell/goneRoutes";
import { restoreClosedSide, restoreSide } from "@/lib/shell/sideStack";
import { recentKey } from "@/stores/shell/recentTabsStore";
import { restore } from "@/stores/shell/tabs/persist";

const previous = Object.getOwnPropertyDescriptor(globalThis, "localStorage");

function stored(strip: unknown) {
  const raw = JSON.stringify(strip);
  Object.defineProperty(globalThis, "localStorage", {
    configurable: true,
    value: { getItem: (key: string) => (key === "oculus-tabs" ? raw : null), setItem: () => {} },
  });
}

afterEach(() => {
  if (previous) Object.defineProperty(globalThis, "localStorage", previous);
  else Reflect.deleteProperty(globalThis, "localStorage");
});

describe("a stored /music path is dropped on load", () => {
  test("only the gone path and what lies under it match", () => {
    expect(isGoneRoute("/music")).toBe(true);
    expect(isGoneRoute("/music?from=rail")).toBe(true);
    expect(isGoneRoute("/music/queue")).toBe(true);
    expect(isGoneRoute("/musicals")).toBe(false);
    expect(isGoneRoute("/settings/music")).toBe(false);
    expect(isGoneRoute("/chat")).toBe(false);
  });

  test("a tab on it goes, and the next tab becomes active", () => {
    stored({
      tabs: [
        { id: 1, path: "/music", side: { items: [{ id: 2, path: "/chat" }], front: 2, ratio: 0.5 } },
        { id: 3, path: "/sync" },
      ],
      activeId: 1,
    });
    const { tabs, activeId } = restore();
    expect(tabs.map((t) => t.path)).toEqual(["/sync"]);
    expect(activeId).toBe(3);
  });

  test("a strip of only that tab restores as a fresh strip", () => {
    stored({ tabs: [{ id: 1, path: "/music" }], activeId: 1 });
    expect(restore().tabs.map((t) => t.path)).toEqual(["/chat"]);
  });

  test("a side panel item on it goes, and the panel fronts what is left", () => {
    stored({
      tabs: [
        {
          id: 1,
          path: "/sync",
          side: { items: [{ id: 2, path: "/music" }, { id: 3, path: "/chat" }], front: 2, ratio: 0.5 },
          focus: "side",
        },
      ],
      activeId: 1,
    });
    const [tab] = restore().tabs;
    expect(tab.side?.items.map((i) => i.path)).toEqual(["/chat"]);
    expect(tab.side?.front).toBe(3);
    expect(restoreSide(undefined, { id: 2, path: "/music" }, 1)).toBeNull();
  });

  test("closed tabs on it, or with it in their side panel, drop it", () => {
    stored({
      tabs: [{ id: 1, path: "/sync" }],
      activeId: 1,
      closed: [
        { index: 0, path: "/music", url: null, side: null },
        { index: 0, path: "/chat", url: null, side: ["/music", "/sync"] },
      ],
    });
    expect(restore().closed).toEqual([{ index: 0, path: "/chat", url: null, side: ["/sync"] }]);
    expect(restoreClosedSide(undefined, "/music")).toBeNull();
  });

  test("a Recent row on it has no key, so the trail drops it", () => {
    expect(recentKey("/music")).toBeNull();
    expect(recentKey("/sync")).toBe("/sync");
  });
});

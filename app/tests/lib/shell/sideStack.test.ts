import { describe, expect, test } from "bun:test";
import {
  SIDE_CAP,
  frontItem,
  frontOf,
  pushItem,
  removeItem,
  restoreClosedSide,
  restoreFocus,
  restoreSide,
  sideFromPaths,
  stepItem,
  storeSide,
  type SidePanel,
} from "@/lib/shell/sideStack";

const file = (n: number) => `/subjects/1/file?path=courses%2FX%2Ffiles%2F${n}.pdf`;

/** A side panel built by pushing `paths` in order, ids from 100. */
function pushed(paths: string[]): { side: SidePanel; newId: () => number } {
  let id = 100;
  const newId = () => id++;
  let side: SidePanel | null = null;
  for (const p of paths) side = pushItem(side, p, newId).side;
  return { side: side!, newId };
}

const paths = (side: SidePanel | null) => side?.items.map((i) => i.path) ?? [];

describe("pushing onto the side panel", () => {
  test("a new path opens a fresh item at the head, in front, at half width", () => {
    const { side, hit, dropped } = pushItem(null, file(1), () => 7, { locate: { seq: 1 } });
    expect(hit).toBeNull();
    expect(dropped).toEqual([]);
    expect(side.ratio).toBe(0.5);
    expect(side.front).toBe(7);
    expect(side.items).toEqual([
      { id: 7, path: file(1), canBack: false, canForward: false, viewed: 1, entryState: { locate: { seq: 1 } } },
    ]);
    const next = pushItem(side, file(2), () => 8).side;
    expect(paths(next)).toEqual([file(2), file(1)]);
    expect(next.front).toBe(8);
  });

  test("a new panel takes the remembered ratio; an open one keeps its own", () => {
    const side = pushItem(null, file(1), () => 7, undefined, 0.65).side;
    expect(side.ratio).toBe(0.65);
    expect(pushItem({ ...side, ratio: 0.4 }, file(2), () => 8, undefined, 0.65).side.ratio).toBe(0.4);
    expect(sideFromPaths(["/a"], () => 9, 0.3)!.ratio).toBe(0.3);
  });

  test("the same thing comes to the front and the head instead of duplicating", () => {
    const { side, newId } = pushed([file(1), file(2), file(3)]);
    const { side: next, hit } = pushItem(side, file(1), newId);
    expect(hit?.id).toBe(100);
    expect(hit?.path).toBe(file(1));
    expect(next.items.map((i) => i.id)).toEqual([100, 102, 101]);
    expect(next.front).toBe(100);
    expect(frontOf(next).viewed).toBeGreaterThan(Math.max(...side.items.map((i) => i.viewed)));

    // A lecture's `t=` title is not part of its identity (`recentKey`).
    const lecture = pushed(["/subjects/1/lecture?id=a&t=Old"]).side;
    expect(pushItem(lecture, "/subjects/1/lecture?id=a&t=New", () => 1).hit?.id).toBe(100);
  });

  test("new-tab and browser pages are never the same thing twice", () => {
    const { side, newId } = pushed(["/new", "/browse/4"]);
    expect(pushItem(side, "/new", newId).hit).toBeNull();
    expect(pushItem(side, "/browse/4", newId).hit).toBeNull();
  });

  test("past the cap the least recently viewed goes, not the least recently opened", () => {
    const { side, newId } = pushed(Array.from({ length: SIDE_CAP }, (_, i) => file(i)));
    // The oldest opened (id 100) is viewed again; 101 is now the stalest.
    const viewed = frontItem(side, 100);
    expect(paths(viewed)).toEqual(paths(side));
    const { side: next, dropped } = pushItem(viewed, file(99), newId);
    expect(dropped.map((i) => i.id)).toEqual([101]);
    expect(next.items).toHaveLength(SIDE_CAP);
    expect(next.items.some((i) => i.id === 100)).toBe(true);
    expect(frontOf(next).path).toBe(file(99));
  });
});

describe("bringing forward and removing", () => {
  test("bringing an item forward keeps the list order and stamps it", () => {
    const { side } = pushed([file(1), file(2), file(3)]);
    const next = frontItem(side, 100);
    expect(next.front).toBe(100);
    expect(next.items.map((i) => i.id)).toEqual([102, 101, 100]);
    expect(frontItem(next, 100)).toBe(next);
    expect(frontItem(next, 999)).toBe(next);
  });

  test("stepping walks the list order from the front, wrapping at both ends", () => {
    // List order 102, 101, 100; bringing one forward does not reorder it.
    const side = frontItem(pushed([file(1), file(2), file(3)]).side, 101);
    expect(stepItem(side, 1)).toBe(100);
    expect(stepItem(side, -1)).toBe(102);
    expect(stepItem(frontItem(side, 100), 1)).toBe(102);
    expect(stepItem(frontItem(side, 102), -1)).toBe(100);
    const one = pushed([file(1)]).side;
    expect(stepItem(one, 1)).toBe(100);
    expect(stepItem(one, -1)).toBe(100);
  });

  test("removing the front hands it to the most recently viewed item left", () => {
    const { side } = pushed([file(1), file(2), file(3)]);
    const viewed = frontItem(frontItem(side, 100), 102);
    // Viewed order, newest first: 102, 100, 101.
    const next = removeItem(viewed, 102)!;
    expect(next.front).toBe(100);
    expect(next.items.map((i) => i.id)).toEqual([101, 100]);
    expect(removeItem(next, 101)!.front).toBe(100);
  });

  test("removing the last item closes the side panel", () => {
    const { side } = pushed([file(1)]);
    expect(removeItem(side, 100)).toBeNull();
    expect(removeItem(side, 5)).toBe(side);
  });
});

describe("restoring", () => {
  test("a stored side panel round-trips, front and ratio included", () => {
    const { side } = pushed([file(1), file(2), file(3)]);
    const stored = frontItem({ ...side, ratio: 0.6 }, 101);
    const back = restoreSide(JSON.parse(JSON.stringify(storeSide(stored))), undefined, 1)!;
    expect(paths(back)).toEqual(paths(stored));
    expect(back.front).toBe(101);
    expect(back.ratio).toBe(0.6);
    // Stamps follow the list after the front, so the cap drops from its end.
    const full = pushed(Array.from({ length: SIDE_CAP }, (_, i) => file(i))).side;
    const restored = restoreSide(storeSide(full), undefined, 1)!;
    expect(pushItem(restored, file(99), () => 1).dropped.map((i) => i.path)).toEqual([file(0)]);
  });

  test("a stored split pane becomes a one-item side panel, its focus the side", () => {
    const side = restoreSide(undefined, { id: 4, path: "/chat" }, 1)!;
    expect(side).toEqual({
      items: [{ id: 4, path: "/chat", canBack: false, canForward: false, viewed: 1 }],
      front: 4,
      ratio: 0.5,
    });
    expect(restoreFocus("split", side)).toBe("side");
    expect(restoreFocus("side", side)).toBe("side");
    expect(restoreFocus("side", null)).toBe("main");
    expect(restoreFocus("main", side)).toBe("main");
  });

  test("anything malformed is no side panel", () => {
    expect(restoreSide(undefined, undefined, 1)).toBeNull();
    expect(restoreSide(null, null, 1)).toBeNull();
    expect(restoreSide({ items: "x" }, undefined, 1)).toBeNull();
    expect(restoreSide({ items: [] }, undefined, 1)).toBeNull();
    expect(restoreSide({ items: [{ id: 2 }] }, undefined, 1)).toBeNull();
    expect(restoreSide({ items: [{ id: "2", path: "/chat" }] }, undefined, 1)).toBeNull();
    expect(restoreSide([{ id: 2, path: "/chat" }], undefined, 1)).toBeNull();
    // An id used twice, or the main pane's, would share a router.
    expect(restoreSide({ items: [{ id: 2, path: "/a" }, { id: 2, path: "/b" }] }, undefined, 1)).toBeNull();
    expect(restoreSide({ items: [{ id: 1, path: "/a" }] }, undefined, 1)).toBeNull();
    expect(restoreSide(undefined, { id: 1, path: "/a" }, 1)).toBeNull();
    expect(restoreSide(undefined, "/chat", 1)).toBeNull();
    // A bad front or ratio falls back rather than failing.
    const side = restoreSide({ items: [{ id: 2, path: "/a" }], front: 9, ratio: "wide" }, undefined, 1)!;
    expect(side.front).toBe(2);
    expect(side.ratio).toBe(0.5);
    expect(restoreSide({ items: [{ id: 2, path: "/a" }], ratio: 0.99 }, undefined, 1)!.ratio).toBe(0.75);
  });

  test("a closed tab's side panel reads either stored shape", () => {
    expect(restoreClosedSide(["/a", "/b"], undefined)).toEqual(["/a", "/b"]);
    expect(restoreClosedSide(undefined, "/a")).toEqual(["/a"]);
    expect(restoreClosedSide(null, null)).toBeNull();
    expect(restoreClosedSide([], undefined)).toBeNull();
    expect(restoreClosedSide([3, "/a"], undefined)).toEqual(["/a"]);
  });

  test("a reopened side panel gets fresh ids with the first path in front", () => {
    let id = 50;
    const side = sideFromPaths(["/a", "/b"], () => id++)!;
    expect(side.items.map((i) => [i.id, i.path])).toEqual([[50, "/a"], [51, "/b"]]);
    expect(side.front).toBe(50);
    expect(side.ratio).toBe(0.5);
    expect(sideFromPaths([], () => id++)).toBeNull();
  });
});

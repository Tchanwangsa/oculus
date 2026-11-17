import { expect, test } from "bun:test";
import { followClosed } from "@/stores/shell/tabs/follow";
import { followRecents, recentKey } from "@/stores/shell/recentTabsStore";
import type { FollowedRoute } from "@/lib/shell/pathFollow";

const follow = (route: string): FollowedRoute =>
  route === "/a" ? { kind: "moved", path: "/b" }
  : route === "/gone" ? { kind: "removed", fallback: "/folder" }
  : { kind: "same" };

test("the reopen stack follows: main paths move or fall back, side paths move or drop", () => {
  const closed = [
    { index: 0, path: "/a", url: null, side: ["/gone", "/a", "/x"] },
    { index: 1, path: "/gone", url: null, side: ["/gone"] },
    { index: 2, path: null, url: "https://x", side: null },
  ];
  expect(followClosed(closed, follow)).toEqual([
    { index: 0, path: "/b", url: null, side: ["/b", "/x"] },
    { index: 1, path: "/folder", url: null, side: null },
    { index: 2, path: null, url: "https://x", side: null },
  ]);
  const same = [{ index: 0, path: "/x", url: null, side: ["/y"] }];
  expect(followClosed(same, follow)).toBe(same);
});

test("Recent rows move in place, merge into a row on the same thing, and drop when deleted", () => {
  const rows = (paths: [string, number][]) => paths.map(([path, visitedAt]) => ({ key: recentKey(path)!, path, visitedAt }));
  const moveFile = (route: string): FollowedRoute =>
    route.includes("old.pdf") ? { kind: "moved", path: route.replace("old.pdf", "new.pdf") }
    : route.includes("gone.pdf") ? { kind: "removed", fallback: "/subjects/1/files" }
    : { kind: "same" };
  const old = "/subjects/1/file?path=courses%2FX%2Fold.pdf";
  const now = "/subjects/1/file?path=courses%2FX%2Fnew.pdf";
  const gone = "/subjects/1/file?path=courses%2FX%2Fgone.pdf";
  const out = followRecents(rows([[now, 1], [old, 5], [gone, 3], ["/calendar", 2]]), moveFile);
  expect(out.map((r) => [r.path, r.visitedAt])).toEqual([[now, 5], ["/calendar", 2]]);
  const list = rows([["/calendar", 2]]);
  expect(followRecents(list, moveFile)).toBe(list);
});

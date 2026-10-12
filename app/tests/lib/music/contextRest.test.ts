import { expect, test } from "bun:test";
import type { MusicNextTrack } from "@/lib/music";
import { continuable, contextRestStart } from "@/lib/music/contextRest";

const next = (uri: string, queued = false): MusicNextTrack => ({ uri, queued });

test("the rest starts after the last track Spotify lists from the context", () => {
  const context = ["a", "b", "c", "d", "e"];
  expect(contextRestStart(context, [next("b"), next("c")], "a")).toBe(3);
  // Queued tracks are not the anchor.
  expect(contextRestStart(context, [next("b"), next("x", true)], "a")).toBe(2);
  expect(contextRestStart(context, [next("e")], "d")).toBe(5);
});

test("a repeated anchor is taken after the current track, else its first place", () => {
  const context = ["a", "b", "c", "a", "b", "c"];
  expect(contextRestStart(context, [next("b")], "c")).toBe(5);
  expect(contextRestStart(context, [next("b")], "a")).toBe(2);
  // The current track is not in the context, or is unknown.
  expect(contextRestStart(context, [next("b")], "z")).toBe(2);
  expect(contextRestStart(context, [next("b")], null)).toBe(2);
});

test("nothing follows without an anchor, or one the context does not list", () => {
  expect(contextRestStart(["a", "b"], [], "a")).toBe(null);
  expect(contextRestStart(["a", "b"], [next("b", true)], "a")).toBe(null);
  expect(contextRestStart(["a", "b"], [next("z")], "a")).toBe(null);
});

test("only playlists and albums continue", () => {
  expect(continuable("spotify:playlist:37i9dQZF1DXcBWIGoYBM5M")).toBe(true);
  expect(continuable("spotify:album:4aawyAB9vmqN3uQ7FjRGTy")).toBe(true);
  expect(continuable("spotify:artist:0OdUWJ0sBjDrqHygGUXeCF")).toBe(false);
  expect(continuable("spotify:collection:tracks")).toBe(false);
  expect(continuable(null)).toBe(false);
});

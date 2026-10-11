import { expect, test } from "bun:test";
import { positionNow, type MusicSnapshot } from "@/lib/music";

const base: MusicSnapshot = {
  connected: true,
  connecting: false,
  needs_login: false,
  account: "a",
  device_name: "Oculus",
  track: {
    uri: "spotify:track:x",
    name: "x",
    artists: [],
    album: "",
    cover_url: null,
    duration_ms: 10_000,
  },
  position_ms: 4_000,
  position_at_ms: 1_000,
  playing: true,
  volume: 50,
  shuffle: false,
  repeat: false,
  error: null,
};

test("the position moves on from its stamp while playing, up to the track's end", () => {
  expect(positionNow(base, 3_000)).toBe(6_000);
  expect(positionNow(base, 60_000)).toBe(10_000);
  expect(positionNow({ ...base, playing: false }, 3_000)).toBe(4_000);
  expect(positionNow(base, 500)).toBe(4_000);
});

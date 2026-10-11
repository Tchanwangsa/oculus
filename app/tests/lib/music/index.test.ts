import { expect, test } from "bun:test";
import {
  musicControllable,
  musicEnabled,
  musicRow,
  positionNow,
  trackPages,
  trackSubtitle,
  type MusicSnapshot,
} from "@/lib/music";

const base: MusicSnapshot = {
  connected: true,
  connecting: false,
  needs_login: false,
  account: "a",
  device_name: "Oculus",
  active: true,
  playing_on: null,
  output: null,
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
  up_next: [],
  error: null,
};

test("the position moves on from its stamp while playing, up to the track's end", () => {
  expect(positionNow(base, 3_000)).toBe(6_000);
  expect(positionNow(base, 60_000)).toBe(10_000);
  expect(positionNow({ ...base, playing: false }, 3_000)).toBe(4_000);
  expect(positionNow(base, 500)).toBe(4_000);
});

test("the settings row keeps every key and reads junk as empty", () => {
  expect(musicRow('{"enabled":true,"output":"AirPods","later":1}')).toEqual({
    enabled: true,
    output: "AirPods",
    later: 1,
  });
  expect(musicRow(null)).toEqual({});
  expect(musicRow("[1]")).toEqual({});
  expect(musicRow("nope")).toEqual({});
  expect(musicEnabled('{"enabled":true}')).toBe(true);
  expect(musicEnabled('{"enabled":"yes"}')).toBe(false);
});

test("the transport acts on a track here, or on another device it would pull from", () => {
  expect(musicControllable(base)).toBe(true);
  const elsewhere = { ...base, track: null, active: false };
  expect(musicControllable(elsewhere)).toBe(false);
  expect(musicControllable({ ...elsewhere, playing_on: "Pixel" })).toBe(true);
  expect(musicControllable({ ...base, connected: false })).toBe(false);
});

test("a track row reads artists · album, leaving out what is missing", () => {
  const row = {
    uri: "spotify:track:x",
    name: "So What",
    artists: ["Miles Davis", "John Coltrane"],
    album: "Kind of Blue",
    image_url: null,
    duration_ms: 1,
  };
  expect(trackSubtitle(row)).toBe("Miles Davis, John Coltrane · Kind of Blue");
  expect(trackSubtitle({ ...row, album: "" })).toBe("Miles Davis, John Coltrane");
  expect(trackSubtitle({ ...row, artists: [] })).toBe("Kind of Blue");
});

test("visible rows map to the hundred-track pages that hold them", () => {
  expect(trackPages(0, 12)).toEqual([0]);
  expect(trackPages(95, 110)).toEqual([0, 1]);
  expect(trackPages(250, 260)).toEqual([2]);
  expect(trackPages(5, 4)).toEqual([]);
  expect(trackPages(-1, 3)).toEqual([]);
});

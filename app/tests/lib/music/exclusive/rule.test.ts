import { expect, test } from "bun:test";
import { toSilence } from "@/lib/music/exclusive/rule";

const s = (music: boolean, local: boolean) => ({ music, local });

test("music starting pauses a lecture or library video that was playing", () => {
  expect(toSilence(s(false, true), s(true, true))).toBe("local");
});

test("a lecture or library video starting pauses music that was playing", () => {
  expect(toSilence(s(true, false), s(true, true))).toBe("music");
});

test("both starting at once keeps the app's own sound", () => {
  expect(toSilence(s(false, false), s(true, true))).toBe("music");
});

test("nothing is paused when the other side is silent", () => {
  expect(toSilence(s(false, false), s(true, false))).toBeNull();
  expect(toSilence(s(false, false), s(false, true))).toBeNull();
});

test("stopping never resumes the other side", () => {
  expect(toSilence(s(true, false), s(false, false))).toBeNull();
  expect(toSilence(s(false, true), s(false, false))).toBeNull();
  // Both already playing (a pause that failed) is left alone, not retried.
  expect(toSilence(s(true, true), s(true, true))).toBeNull();
});

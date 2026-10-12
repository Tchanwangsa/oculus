import { expect, test } from "bun:test";

import { DEFAULT_STYLE, lengthLabel, readStyle, writeStyle } from "@/components/studio/style/style";
import { formatLength, lengthInput, parseLength } from "@/lib/studio/length";

const secs = (text: string) => {
  const p = parseLength(text);
  return "secs" in p ? p.secs : p.problem;
};

test("a typed length reads as seconds, minutes or a clock", () => {
  expect(secs("45")).toBe(45);
  expect(secs("90s")).toBe(90);
  expect(secs(" 90 sec ")).toBe(90);
  expect(secs("3m")).toBe(180);
  expect(secs("2 min")).toBe(120);
  expect(secs("1.5m")).toBe(90);
  expect(secs("1:30")).toBe(90);
  expect(secs("10:05")).toBe(605);
  expect(secs("5")).toBe(5);
});

test("a length that is not a usable number says what is wrong", () => {
  expect(secs("")).toContain("Type a length");
  expect(secs("soon")).toContain("Type a length");
  expect(secs("3x")).toContain("Type a length");
  expect(secs("1:75")).toContain("Type a length");
  expect(secs("-30")).toContain("Type a length");
  expect(secs("4")).toContain("at least 5");
  expect(secs("0")).toContain("at least 5");
  expect(secs("0:03")).toContain("at least 5");
  expect(secs("100000")).toContain("longer than a day");
});

test("a length prints the way it can be typed back", () => {
  expect(formatLength(30)).toBe("30 s");
  expect(formatLength(90)).toBe("90 s");
  expect(formatLength(120)).toBe("2 min");
  expect(formatLength(150)).toBe("2:30");
  expect(formatLength(305)).toBe("5:05");
  for (const n of [5, 45, 90, 120, 150, 300, 605]) expect(secs(lengthInput(n))).toBe(n);
});

test("the style chip reads Auto when no length is set", () => {
  expect(lengthLabel(null)).toBe("Auto");
  expect(lengthLabel(60)).toBe("60 s");
  expect(lengthLabel(300)).toBe("5 min");
});

test("a fresh install has no length target", () => {
  expect(DEFAULT_STYLE.lengthSecs).toBeNull();
  expect(readStyle(null).lengthSecs).toBeNull();
  expect(readStyle("not json").lengthSecs).toBeNull();
});

test("a stored length reads back, preset or custom, and Auto stays Auto", () => {
  for (const n of [30, 60, 120, 300, 45, 90, 755]) {
    const stored = writeStyle({ theme: null, format: "reels", lengthSecs: n });
    expect(readStyle(stored)).toEqual({ theme: null, format: "reels", lengthSecs: n });
  }
  const auto = writeStyle({ theme: null, format: "video", lengthSecs: null });
  expect(readStyle(auto).lengthSecs).toBeNull();
  expect(readStyle('{"format":"video"}').lengthSecs).toBeNull();
  expect(readStyle('{"lengthSecs":"60"}').lengthSecs).toBeNull();
  expect(readStyle('{"lengthSecs":-5}').lengthSecs).toBeNull();
  expect(readStyle('{"lengthSecs":0}').lengthSecs).toBeNull();
});

import { describe, expect, test } from "bun:test";
import { hsvToRgb, parseHex, rgbToHsv, toHex, unit } from "@/lib/ui/color";

describe("parseHex", () => {
  test("reads the three forms a theme accepts, with or without #", () => {
    expect(parseHex("#5e6ad2")).toEqual({ r: 0x5e, g: 0x6a, b: 0xd2, a: 1 });
    expect(parseHex("fff")).toEqual({ r: 255, g: 255, b: 255, a: 1 });
    expect(parseHex("#16161d26")).toEqual({ r: 0x16, g: 0x16, b: 0x1d, a: 0x26 / 255 });
    expect(parseHex(" #ABC ")).toEqual({ r: 0xaa, g: 0xbb, b: 0xcc, a: 1 });
  });

  test("refuses anything else", () => {
    for (const bad of ["", "#", "#12", "#12345", "#1234567", "blue", "rgb(1,2,3)", "#ggg"]) {
      expect(parseHex(bad)).toBeNull();
    }
  });
});

describe("toHex", () => {
  test("writes six digits, and an alpha pair only when not opaque", () => {
    expect(toHex({ r: 94, g: 106, b: 210, a: 1 })).toBe("#5e6ad2");
    expect(toHex({ r: 22, g: 22, b: 29, a: 0x26 / 255 })).toBe("#16161d26");
    expect(toHex({ r: 300, g: -4, b: 12.4, a: 1 })).toBe("#ff000c");
  });

  test("round-trips through parseHex", () => {
    for (const hex of ["#000000", "#ffffff", "#5e6ad2", "#16161d26", "#c2410c"]) {
      expect(toHex(parseHex(hex)!)).toBe(hex);
    }
  });
});

describe("HSV", () => {
  test("primaries and greys", () => {
    expect(rgbToHsv({ r: 255, g: 0, b: 0, a: 1 })).toEqual({ h: 0, s: 1, v: 1 });
    expect(rgbToHsv({ r: 0, g: 255, b: 0, a: 1 })).toEqual({ h: 120, s: 1, v: 1 });
    expect(rgbToHsv({ r: 0, g: 0, b: 255, a: 1 })).toEqual({ h: 240, s: 1, v: 1 });
    expect(rgbToHsv({ r: 128, g: 128, b: 128, a: 1 })).toEqual({ h: 0, s: 0, v: 128 / 255 });
    expect(hsvToRgb({ h: 0, s: 0, v: 0 })).toEqual({ r: 0, g: 0, b: 0, a: 1 });
    expect(hsvToRgb({ h: 360, s: 1, v: 1 }, 0.5)).toEqual({ r: 255, g: 0, b: 0, a: 0.5 });
  });

  test("every theme colour survives a trip through HSV", () => {
    for (const hex of ["#5e6ad2", "#f4f4f6", "#2f9e6e", "#d99a2b", "#d1495b", "#7a6556", "#fde9dc"]) {
      const rgba = parseHex(hex)!;
      expect(toHex(hsvToRgb(rgbToHsv(rgba), rgba.a))).toBe(hex);
    }
  });

  test("unit clamps", () => {
    expect([unit(-1), unit(0.25), unit(3)]).toEqual([0, 0.25, 1]);
  });
});

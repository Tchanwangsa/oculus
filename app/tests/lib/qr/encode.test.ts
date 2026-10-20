import { describe, expect, test } from "bun:test";
import { byteCapacity, encodeQr, formatBits, MAX_VERSION, qrPath, QUIET_ZONE, versionBits } from "@/lib/qr";
import { LINKS } from "../../qr/links";

const link = LINKS[2];

function finderAt(m: boolean[][], top: number, left: number) {
  for (let r = 0; r < 7; r++) for (let c = 0; c < 7; c++) {
    const ring = Math.max(Math.abs(r - 3), Math.abs(c - 3));
    expect(m[top + r][left + c]).toBe(ring !== 2);
  }
}

describe("QR encoder", () => {
  test("the matrix is 17 + 4·version modules square, smallest version that fits", () => {
    for (const text of LINKS) {
      const code = encodeQr(text);
      expect(code.size).toBe(17 + 4 * code.version);
      expect(code.modules.length).toBe(code.size);
      for (const row of code.modules) expect(row.length).toBe(code.size);
      const bytes = new TextEncoder().encode(text).length;
      expect(byteCapacity(code.version)).toBeGreaterThanOrEqual(bytes);
      if (code.version > 1) expect(byteCapacity(code.version - 1)).toBeLessThan(bytes);
    }
  });

  test("byte capacities at level M match the spec's table", () => {
    expect([1, 2, 3, 4, 5, 6, 7, 8, 9, 10].map(byteCapacity)).toEqual([14, 26, 42, 62, 84, 106, 122, 152, 180, 213]);
  });

  test("three finders with light separators sit in the corners", () => {
    const { modules: m, size } = encodeQr(link);
    finderAt(m, 0, 0);
    finderAt(m, 0, size - 7);
    finderAt(m, size - 7, 0);
    for (let i = 0; i < 8; i++) {
      expect(m[7][i]).toBe(false);
      expect(m[i][7]).toBe(false);
      expect(m[7][size - 1 - i]).toBe(false);
      expect(m[size - 8][i]).toBe(false);
    }
  });

  test("timing patterns alternate between the finders", () => {
    const { modules: m, size } = encodeQr(link);
    for (let i = 8; i < size - 8; i++) {
      expect(m[6][i]).toBe(i % 2 === 0);
      expect(m[i][6]).toBe(i % 2 === 0);
    }
  });

  test("alignment patterns sit at the spec's centres", () => {
    const code = encodeQr(LINKS[7]);
    expect(code.version).toBe(8);
    for (const [r, c] of [[24, 24], [6, 24], [24, 6], [24, 42], [42, 24], [42, 42]]) {
      for (let dr = -2; dr <= 2; dr++) for (let dc = -2; dc <= 2; dc++) {
        expect(code.modules[r + dr][c + dc]).toBe(Math.max(Math.abs(dr), Math.abs(dc)) !== 1);
      }
    }
  });

  test("both copies of the format bits name level M and the chosen mask", () => {
    for (const text of LINKS) {
      const { modules: m, size, mask } = encodeQr(text);
      const bits = formatBits(mask);
      const at = (i: number) => ((bits >>> i) & 1) === 1;
      const first = [m[0][8], m[1][8], m[2][8], m[3][8], m[4][8], m[5][8], m[7][8], m[8][8], m[8][7], m[8][5], m[8][4], m[8][3], m[8][2], m[8][1], m[8][0]];
      const second = [...Array.from({ length: 8 }, (_, i) => m[8][size - 1 - i]), ...Array.from({ length: 7 }, (_, i) => m[size - 7 + i][8])];
      for (let i = 0; i < 15; i++) {
        expect(first[i]).toBe(at(i));
        expect(second[i]).toBe(at(i));
      }
      // Unmasked, the top two data bits are M's 00 and the next three the mask.
      expect((bits ^ 0x5412) >>> 10).toBe(mask);
      expect(m[size - 8][8]).toBe(true);
    }
  });

  test("format and version words match the spec's published values", () => {
    // ISO/IEC 18004 Annex C: level M, mask 0 → 101010000010010; version 7 → 000111110010010100.
    expect(formatBits(0)).toBe(0b101010000010010);
    expect(versionBits(7)).toBe(0b000111110010010100);
    expect(versionBits(10)).toBe(0b001010010011010011);
  });

  test("versions 7 and up carry both version blocks", () => {
    const code = encodeQr(LINKS[4]);
    expect(code.version).toBe(7);
    const bits = versionBits(7);
    for (let i = 0; i < 18; i++) {
      const a = code.size - 11 + (i % 3), b = Math.floor(i / 3);
      const want = ((bits >>> i) & 1) === 1;
      expect(code.modules[b][a]).toBe(want);
      expect(code.modules[a][b]).toBe(want);
    }
  });

  test("the same text always gives the same code", () => {
    expect(encodeQr(link)).toEqual(encodeQr(link));
  });

  test("text past version 10 is refused", () => {
    expect(() => encodeQr("x".repeat(byteCapacity(MAX_VERSION) + 1))).toThrow(/Too long/);
  });

  test("the SVG path covers every dark module inside a 4-module quiet zone", () => {
    const { modules, size } = encodeQr(link);
    const { extent, d } = qrPath(modules);
    expect(extent).toBe(size + 2 * QUIET_ZONE);
    let covered = 0;
    for (const [, x, y, w] of d.matchAll(/M(\d+) (\d+)h(\d+)v1h-\d+z/g)) {
      for (let k = 0; k < Number(w); k++) {
        expect(modules[Number(y) - QUIET_ZONE][Number(x) - QUIET_ZONE + k]).toBe(true);
        covered++;
      }
    }
    expect(covered).toBe(modules.flat().filter(Boolean).length);
  });
});

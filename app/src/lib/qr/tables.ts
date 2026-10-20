/** ISO/IEC 18004 constants for error correction level M, versions 1–10: the
 * Reed–Solomon block layout and the alignment pattern centres. */

export const MIN_VERSION = 1;
export const MAX_VERSION = 10;

/** Per version: EC codewords per block, then `[blocks, data codewords]` groups. */
export const BLOCKS_M: Record<number, { ec: number; groups: readonly (readonly [number, number])[] }> = {
  1: { ec: 10, groups: [[1, 16]] },
  2: { ec: 16, groups: [[1, 28]] },
  3: { ec: 26, groups: [[1, 44]] },
  4: { ec: 18, groups: [[2, 32]] },
  5: { ec: 24, groups: [[2, 43]] },
  6: { ec: 16, groups: [[4, 27]] },
  7: { ec: 18, groups: [[4, 31]] },
  8: { ec: 22, groups: [[2, 38], [2, 39]] },
  9: { ec: 22, groups: [[3, 36], [2, 37]] },
  10: { ec: 26, groups: [[4, 43], [1, 44]] },
};

export const ALIGNMENT: Record<number, readonly number[]> = {
  1: [],
  2: [6, 18],
  3: [6, 22],
  4: [6, 26],
  5: [6, 30],
  6: [6, 34],
  7: [6, 22, 38],
  8: [6, 24, 42],
  9: [6, 26, 46],
  10: [6, 28, 50],
};

export const dataCodewords = (version: number) =>
  BLOCKS_M[version].groups.reduce((sum, [blocks, size]) => sum + blocks * size, 0);

/** The byte-mode character count field is 8 bits up to version 9, 16 from 10. */
export const countBits = (version: number) => (version <= 9 ? 8 : 16);

/** Bytes that fit in byte mode: the 4-bit mode and the count field come first. */
export const byteCapacity = (version: number) => Math.floor((dataCodewords(version) * 8 - 4 - countBits(version)) / 8);

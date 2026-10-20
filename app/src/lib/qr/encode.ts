/** A QR code encoder with no dependency, after ISO/IEC 18004: byte mode, error
 * correction M, versions 1–10 (up to 213 bytes, plenty for a share link). It
 * picks the smallest version that fits and the mask with the lowest penalty.
 * The share dialog draws links with it, so a link never leaves the Mac to be
 * turned into a picture. */
import { reedSolomon } from "./reedSolomon";
import { ALIGNMENT, BLOCKS_M, MAX_VERSION, MIN_VERSION, byteCapacity, countBits, dataCodewords } from "./tables";

export interface QrCode {
  version: number;
  mask: number;
  size: number;
  /** `modules[row][column]`, true for dark. */
  modules: boolean[][];
}

/** Error correction M's two format bits are 00. */
const FORMAT_LEVEL_M = 0b00;

const MASKS: readonly ((row: number, col: number) => boolean)[] = [
  (r, c) => (r + c) % 2 === 0,
  (r) => r % 2 === 0,
  (_r, c) => c % 3 === 0,
  (r, c) => (r + c) % 3 === 0,
  (r, c) => (Math.floor(r / 2) + Math.floor(c / 3)) % 2 === 0,
  (r, c) => ((r * c) % 2) + ((r * c) % 3) === 0,
  (r, c) => (((r * c) % 2) + ((r * c) % 3)) % 2 === 0,
  (r, c) => (((r + c) % 2) + ((r * c) % 3)) % 2 === 0,
];

/** The 15 format bits for level M and `mask`, BCH-protected and masked with 0x5412. */
export function formatBits(mask: number): number {
  const data = (FORMAT_LEVEL_M << 3) | mask;
  let rem = data;
  for (let i = 0; i < 10; i++) rem = (rem << 1) ^ ((rem >>> 9) * 0x537);
  return ((data << 10) | rem) ^ 0x5412;
}

/** The 18 version bits (versions 7 and up). */
export function versionBits(version: number): number {
  let rem = version;
  for (let i = 0; i < 12; i++) rem = (rem << 1) ^ ((rem >>> 11) * 0x1f25);
  return (version << 12) | rem;
}

const bit = (value: number, index: number) => ((value >>> index) & 1) !== 0;

export function encodeQr(text: string): QrCode {
  const bytes = new TextEncoder().encode(text);
  let version = MIN_VERSION;
  while (version <= MAX_VERSION && byteCapacity(version) < bytes.length) version++;
  if (version > MAX_VERSION) throw new Error(`Too long for a QR code here (${bytes.length} bytes; at most ${byteCapacity(MAX_VERSION)}).`);
  const codewords = interleave(version, dataStream(version, bytes));
  const base = new Grid(version);
  base.drawFunctionPatterns();
  base.drawCodewords(codewords);
  let best: { grid: Grid; mask: number; penalty: number } | null = null;
  for (let mask = 0; mask < 8; mask++) {
    const grid = base.clone();
    grid.applyMask(mask);
    grid.drawFormat(mask);
    const penalty = grid.penalty();
    if (!best || penalty < best.penalty) best = { grid, mask, penalty };
  }
  return { version, mask: best!.mask, size: base.size, modules: best!.grid.modules };
}

/** Mode, count, data, terminator and padding, as the version's data codewords. */
function dataStream(version: number, bytes: Uint8Array): Uint8Array {
  const bits: number[] = [];
  const push = (value: number, length: number) => { for (let i = length - 1; i >= 0; i--) bits.push((value >>> i) & 1); };
  push(0b0100, 4);
  push(bytes.length, countBits(version));
  for (const byte of bytes) push(byte, 8);
  const capacity = dataCodewords(version) * 8;
  push(0, Math.min(4, capacity - bits.length));
  push(0, (8 - (bits.length % 8)) % 8);
  const out = new Uint8Array(capacity / 8);
  for (let i = 0; i < bits.length; i++) out[i >>> 3] |= bits[i] << (7 - (i & 7));
  for (let i = bits.length / 8, pad = 0xec; i < out.length; i++, pad ^= 0xec ^ 0x11) out[i] = pad;
  return out;
}

/** Splits into blocks, adds each block's EC codewords, and interleaves both. */
function interleave(version: number, data: Uint8Array): Uint8Array {
  const { ec, groups } = BLOCKS_M[version];
  const blocks: { data: Uint8Array; ec: Uint8Array }[] = [];
  let offset = 0;
  for (const [count, size] of groups) {
    for (let i = 0; i < count; i++) {
      const block = data.subarray(offset, offset + size);
      offset += size;
      blocks.push({ data: block, ec: reedSolomon(block, ec) });
    }
  }
  const out: number[] = [];
  const longest = Math.max(...blocks.map((b) => b.data.length));
  for (let i = 0; i < longest; i++) for (const b of blocks) if (i < b.data.length) out.push(b.data[i]);
  for (let i = 0; i < ec; i++) for (const b of blocks) out.push(b.ec[i]);
  return Uint8Array.from(out);
}

class Grid {
  readonly size: number;
  modules: boolean[][];
  reserved: boolean[][];

  constructor(readonly version: number, copy?: Grid) {
    this.size = version * 4 + 17;
    this.modules = copy ? copy.modules.map((row) => [...row]) : Array.from({ length: this.size }, () => Array<boolean>(this.size).fill(false));
    this.reserved = copy ? copy.reserved : Array.from({ length: this.size }, () => Array<boolean>(this.size).fill(false));
  }

  clone() { return new Grid(this.version, this); }

  private set(row: number, col: number, dark: boolean) {
    this.modules[row][col] = dark;
    this.reserved[row][col] = true;
  }

  drawFunctionPatterns() {
    const n = this.size;
    for (let i = 0; i < n; i++) {
      this.set(6, i, i % 2 === 0);
      this.set(i, 6, i % 2 === 0);
    }
    for (const [row, col] of [[3, 3], [3, n - 4], [n - 4, 3]]) this.finder(row, col);
    const centres = ALIGNMENT[this.version];
    const last = centres.length - 1;
    for (let i = 0; i <= last; i++) {
      for (let j = 0; j <= last; j++) {
        // The three corners that hold finders take no alignment pattern.
        if ((i === 0 && j === 0) || (i === 0 && j === last) || (i === last && j === 0)) continue;
        for (let dr = -2; dr <= 2; dr++) for (let dc = -2; dc <= 2; dc++) this.set(centres[i] + dr, centres[j] + dc, Math.max(Math.abs(dr), Math.abs(dc)) !== 1);
      }
    }
    // Reserve the format areas (and the dark module) before data is placed.
    this.drawFormat(0);
    if (this.version >= 7) {
      const bits = versionBits(this.version);
      for (let i = 0; i < 18; i++) {
        const a = n - 11 + (i % 3), b = Math.floor(i / 3);
        this.set(b, a, bit(bits, i));
        this.set(a, b, bit(bits, i));
      }
    }
  }

  /** A 7×7 finder centred at (row, col) with its light separator. */
  private finder(row: number, col: number) {
    for (let dr = -4; dr <= 4; dr++) {
      for (let dc = -4; dc <= 4; dc++) {
        const r = row + dr, c = col + dc;
        if (r < 0 || c < 0 || r >= this.size || c >= this.size) continue;
        const ring = Math.max(Math.abs(dr), Math.abs(dc));
        this.set(r, c, ring !== 2 && ring !== 4);
      }
    }
  }

  drawFormat(mask: number) {
    const bits = formatBits(mask), n = this.size;
    for (let i = 0; i <= 5; i++) this.set(i, 8, bit(bits, i));
    this.set(7, 8, bit(bits, 6));
    this.set(8, 8, bit(bits, 7));
    this.set(8, 7, bit(bits, 8));
    for (let i = 9; i < 15; i++) this.set(8, 14 - i, bit(bits, i));
    for (let i = 0; i < 8; i++) this.set(8, n - 1 - i, bit(bits, i));
    for (let i = 8; i < 15; i++) this.set(n - 15 + i, 8, bit(bits, i));
    this.set(n - 8, 8, true);
  }

  /** Two-column zigzag from the bottom right, skipping the vertical timing column. */
  drawCodewords(codewords: Uint8Array) {
    const n = this.size;
    let i = 0;
    for (let right = n - 1; right >= 1; right -= 2) {
      if (right === 6) right = 5;
      const upward = ((right + 1) & 2) === 0;
      for (let step = 0; step < n; step++) {
        const row = upward ? n - 1 - step : step;
        for (let j = 0; j < 2; j++) {
          const col = right - j;
          if (this.reserved[row][col]) continue;
          // Remainder bits past the last codeword stay light.
          if (i < codewords.length * 8) this.modules[row][col] = bit(codewords[i >>> 3], 7 - (i & 7));
          i++;
        }
      }
    }
  }

  applyMask(mask: number) {
    const test = MASKS[mask];
    for (let r = 0; r < this.size; r++) for (let c = 0; c < this.size; c++) {
      if (!this.reserved[r][c] && test(r, c)) this.modules[r][c] = !this.modules[r][c];
    }
  }

  /** The spec's four penalty rules: runs, 2×2 blocks, finder-like runs, balance. */
  penalty(): number {
    const n = this.size, m = this.modules;
    let score = 0;
    const lines: boolean[][] = [];
    for (let i = 0; i < n; i++) {
      lines.push(m[i]);
      lines.push(m.map((row) => row[i]));
    }
    const finderLike = [true, false, true, true, true, false, true];
    for (const line of lines) {
      let run = 1;
      for (let i = 1; i <= n; i++) {
        if (i < n && line[i] === line[i - 1]) { run++; continue; }
        if (run >= 5) score += 3 + (run - 5);
        run = 1;
      }
      // 1:1:3:1:1 with four light modules on either side; past the edge reads light.
      const at = (i: number) => i >= 0 && i < n && line[i];
      for (let i = 0; i + 7 <= n; i++) {
        if (!finderLike.every((dark, k) => at(i + k) === dark)) continue;
        const before = [1, 2, 3, 4].every((k) => !at(i - k));
        const after = [7, 8, 9, 10].every((k) => !at(i + k));
        if (before) score += 40;
        if (after) score += 40;
      }
    }
    for (let r = 0; r + 1 < n; r++) for (let c = 0; c + 1 < n; c++) {
      const v = m[r][c];
      if (m[r][c + 1] === v && m[r + 1][c] === v && m[r + 1][c + 1] === v) score += 3;
    }
    let dark = 0;
    for (const row of m) for (const v of row) if (v) dark++;
    const percent = (dark * 100) / (n * n);
    score += Math.floor(Math.abs(percent - 50) / 5) * 10;
    return score;
  }
}

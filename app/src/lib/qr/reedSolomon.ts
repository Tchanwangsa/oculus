/** Reed–Solomon error correction over GF(256) with the QR field polynomial
 * x⁸ + x⁴ + x³ + x² + 1 (0x11D) and generator roots α⁰ … αⁿ⁻¹. */

const EXP = new Uint8Array(512);
const LOG = new Uint8Array(256);
{
  let x = 1;
  for (let i = 0; i < 255; i++) {
    EXP[i] = x;
    LOG[x] = i;
    x <<= 1;
    if (x & 0x100) x ^= 0x11d;
  }
  for (let i = 255; i < 512; i++) EXP[i] = EXP[i - 255];
}

const multiply = (a: number, b: number) => (a === 0 || b === 0 ? 0 : EXP[LOG[a] + LOG[b]]);

/** Coefficients of ∏(x − αⁱ), highest degree first, without the leading 1. */
function generator(degree: number): Uint8Array {
  let poly = new Uint8Array([1]);
  for (let i = 0; i < degree; i++) {
    const next = new Uint8Array(poly.length + 1);
    for (let j = 0; j < poly.length; j++) {
      next[j] ^= poly[j];
      next[j + 1] ^= multiply(poly[j], EXP[i]);
    }
    poly = next;
  }
  return poly.subarray(1);
}

/** The `degree` error-correction codewords for one block of data codewords. */
export function reedSolomon(data: Uint8Array, degree: number): Uint8Array {
  const gen = generator(degree);
  const remainder = new Uint8Array(degree);
  for (const byte of data) {
    const factor = byte ^ remainder[0];
    remainder.copyWithin(0, 1);
    remainder[degree - 1] = 0;
    for (let i = 0; i < degree; i++) remainder[i] ^= multiply(gen[i], factor);
  }
  return remainder;
}

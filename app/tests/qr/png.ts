import { deflateSync } from "node:zlib";

const CRC_TABLE = Array.from({ length: 256 }, (_, n) => {
  let c = n;
  for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
  return c >>> 0;
});

function crc32(bytes: Uint8Array): number {
  let c = 0xffffffff;
  for (const b of bytes) c = CRC_TABLE[(c ^ b) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

function chunk(type: string, data: Uint8Array): Uint8Array {
  const out = new Uint8Array(12 + data.length);
  const view = new DataView(out.buffer);
  view.setUint32(0, data.length);
  out.set(new TextEncoder().encode(type), 4);
  out.set(data, 8);
  view.setUint32(8 + data.length, crc32(out.subarray(4, 8 + data.length)));
  return out;
}

/** An 8-bit greyscale PNG of a QR matrix: `scale` pixels per module, a 4-module white border. */
export function qrPng(modules: readonly (readonly boolean[])[], scale = 8): Uint8Array {
  const side = (modules.length + 8) * scale;
  const raw = new Uint8Array(side * (side + 1)).fill(255);
  for (let y = 0; y < side; y++) {
    raw[y * (side + 1)] = 0; // filter: none
    const r = Math.floor(y / scale) - 4;
    for (let x = 0; x < side; x++) {
      const c = Math.floor(x / scale) - 4;
      if (modules[r]?.[c]) raw[y * (side + 1) + 1 + x] = 0;
    }
  }
  const header = new Uint8Array(13);
  const view = new DataView(header.buffer);
  view.setUint32(0, side);
  view.setUint32(4, side);
  header.set([8, 0, 0, 0, 0], 8);
  const parts = [
    Uint8Array.from([137, 80, 78, 71, 13, 10, 26, 10]),
    chunk("IHDR", header),
    chunk("IDAT", new Uint8Array(deflateSync(raw))),
    chunk("IEND", new Uint8Array()),
  ];
  const out = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
  let at = 0;
  for (const p of parts) { out.set(p, at); at += p.length; }
  return out;
}

import { describe, expect, test } from "bun:test";
import { whiteboardImageMime, whiteboardPdfRasterSize, readWhiteboardImages, assertWhiteboardImportSize, MAX_WHITEBOARD_IMPORT_BYTES } from "../../../src/lib/whiteboard/importMedia";

describe("whiteboard media imports", () => {
  test("recognizes raster signatures and refuses SVG or disguised files", () => {
    expect(whiteboardImageMime(new Uint8Array([137, 80, 78, 71, 13, 10, 26, 10]))).toBe("image/png");
    expect(whiteboardImageMime(new Uint8Array([255, 216, 255]))).toBe("image/jpeg");
    expect(whiteboardImageMime(new TextEncoder().encode("GIF89a"))).toBe("image/gif");
    expect(whiteboardImageMime(new TextEncoder().encode("RIFF1234WEBP"))).toBe("image/webp");
    expect(whiteboardImageMime(new TextEncoder().encode("<svg></svg>"))).toBeNull();
    expect(whiteboardImageMime(new Uint8Array())).toBeNull();
  });
  test("PDF previews and imported pages fit the same native pixel limits", () => {
    expect(whiteboardPdfRasterSize(612, 792, true)).toEqual({ width: 185, height: 240 });
    for (const [width, height] of [[612, 792], [1e8, 1], [1, 1e8], [1000, 1000]]) {
      const size = whiteboardPdfRasterSize(width, height);
      expect(size.width).toBeGreaterThan(0);
      expect(size.height).toBeGreaterThan(0);
      expect(Math.max(size.width, size.height)).toBeLessThanOrEqual(2400);
      expect(size.width * size.height).toBeLessThanOrEqual(4_000_000);
    }
    expect(() => whiteboardPdfRasterSize(NaN, 4)).toThrow();
    expect(() => whiteboardPdfRasterSize(0, 4)).toThrow();
  });
  test("bounds an import batch by count and encoded storage", () => {
    const image = { dataUrl: "data:image/png;base64,x", width: 1, height: 1, name: "image" };
    expect(() => assertWhiteboardImportSize(Array(100).fill(image))).not.toThrow();
    expect(() => assertWhiteboardImportSize(Array(101).fill(image))).toThrow("100 images");
    expect(() => assertWhiteboardImportSize([{ ...image, dataUrl: "x".repeat(MAX_WHITEBOARD_IMPORT_BYTES + 1) }])).toThrow("64 MB");
  });
  test("rejects empty and non-raster input before decoding, and honours cancellation", async () => {
    await expect(readWhiteboardImages([new File([], "empty.png")])).rejects.toThrow("24 MB");
    await expect(readWhiteboardImages([new File(["<svg></svg>"], "fake.png", { type: "image/png" })])).rejects.toThrow("PNG, JPEG");
    const controller = new AbortController(); controller.abort();
    await expect(readWhiteboardImages([new File(["x"], "x")], controller.signal)).rejects.toThrow("cancelled");
  });
});

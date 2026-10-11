// Renders the Windows logos from src-tauri/windows/logo.svg: the MSIX
// Assets (scale and taskbar targetsize variants, which MakePri indexes by
// file name) and the icon.ico Tauri embeds in the exe and NSIS installer.
// The outputs are committed; rerun after changing the SVG.
import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const windows = join(resolve(dirname(fileURLToPath(import.meta.url)), ".."), "src-tauri", "windows");
const scales = [100, 125, 150, 200, 400];
// The taskbar, Start and title bar pick the nearest targetsize; "unplated"
// draws the glyph without the accent-coloured plate behind it.
const targetSizes = [16, 20, 24, 30, 32, 36, 40, 48, 60, 64, 72, 80, 96, 256];
const icoSizes = [16, 20, 24, 32, 40, 48, 64, 256];

export function logoFiles() {
  const files = [];
  for (const scale of scales) {
    files.push({ name: `Square44x44Logo.scale-${scale}.png`, size: Math.round(44 * scale / 100), fill: 1 });
    files.push({ name: `Square150x150Logo.scale-${scale}.png`, size: Math.round(150 * scale / 100), fill: 0.6 });
    files.push({ name: `StoreLogo.scale-${scale}.png`, size: Math.round(50 * scale / 100), fill: 1 });
  }
  for (const size of targetSizes) {
    files.push({ name: `Square44x44Logo.targetsize-${size}.png`, size, fill: 1 });
    files.push({ name: `Square44x44Logo.altform-unplated_targetsize-${size}.png`, size, fill: 1 });
  }
  return files;
}

function render(Resvg, svg, size, fill) {
  // Center the glyph at `fill` of the canvas; the tile logo needs margin.
  const inner = Math.round(size * fill), offset = (size - inner) / 2;
  const body = svg.replace(/<!--[\s\S]*?-->/, "").replace(/<svg[^>]*>/, (open) => open
    .replace(/width="[^"]*"/, `width="${inner}"`).replace(/height="[^"]*"/, `height="${inner}"`)
    .replace("<svg", `<svg x="${offset}" y="${offset}"`));
  const page = `<svg xmlns="http://www.w3.org/2000/svg" width="${size}" height="${size}">${body}</svg>`;
  return new Resvg(page, { fitTo: { mode: "original" } }).render().asPng();
}

// ICO with PNG-compressed entries, which Windows Vista and later read.
export function ico(images) {
  const header = Buffer.alloc(6 + 16 * images.length);
  header.writeUInt16LE(1, 2);
  header.writeUInt16LE(images.length, 4);
  let offset = header.length;
  images.forEach(({ size, png }, i) => {
    const entry = 6 + 16 * i;
    header.writeUInt8(size >= 256 ? 0 : size, entry);
    header.writeUInt8(size >= 256 ? 0 : size, entry + 1);
    header.writeUInt16LE(1, entry + 4);
    header.writeUInt16LE(32, entry + 6);
    header.writeUInt32LE(png.length, entry + 8);
    header.writeUInt32LE(offset, entry + 12);
    offset += png.length;
  });
  return Buffer.concat([header, ...images.map(({ png }) => png)]);
}

async function main() {
  // Imported here so the packaging tests can read logoFiles() without it.
  const { Resvg, initWasm } = await import("@resvg/resvg-wasm");
  await initWasm(readFileSync(fileURLToPath(import.meta.resolve("@resvg/resvg-wasm/index_bg.wasm"))));
  const svg = readFileSync(join(windows, "logo.svg"), "utf8");
  const assets = join(windows, "Assets");
  rmSync(assets, { recursive: true, force: true });
  mkdirSync(assets);
  for (const { name, size, fill } of logoFiles()) writeFileSync(join(assets, name), render(Resvg, svg, size, fill));
  writeFileSync(join(windows, "icon.ico"), ico(icoSizes.map((size) => ({ size, png: render(Resvg, svg, size, 1) }))));
  console.log(`Wrote ${logoFiles().length} logos and icon.ico to ${windows}`);
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) await main();

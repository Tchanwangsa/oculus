import { defineConfig, type Plugin } from "vite";
import react from "@vitejs/plugin-react";
import { readdirSync, readFileSync } from "node:fs";
import path from "node:path";

/**
 * The kit chat widgets run on (docs/widgets.md), built to `dist/widget-kit/`
 * after the desktop bundle and served by the media server under
 * `/widget-kit/`. One ES entry per import-map name, sharing one React chunk,
 * so a widget's `react` and the kit's are the same module. Entry names are
 * fixed (the import map names them); chunks and assets are hashed.
 * `oculus/2d`, `oculus/math` and `oculus/3d` are entries of their own, so a
 * widget fetches them only when it imports them; `three` and
 * `@react-three/fiber` (`three.js`, `fiber.js`) share their chunks with
 * `3d.js`, so a frame holds one three and one React. `ask.js` shares its
 * pending-request map with `runtime.js`, which hands it the parent's replies.
 */

const kit = path.resolve(__dirname, "widget-kit");
const fontsource = path.resolve(__dirname, "node_modules/@fontsource-variable");
const katex = path.resolve(__dirname, "src/styles/katex");

/** Faces `kit.css` declares; nothing else is copied. */
const FONTS = [
  "inter/files/inter-latin-wght-normal.woff2",
  "inter/files/inter-latin-ext-wght-normal.woff2",
  "inter/files/inter-greek-wght-normal.woff2",
  "manrope/files/manrope-latin-wght-normal.woff2",
  "manrope/files/manrope-latin-ext-wght-normal.woff2",
];

/** `kit.css`, its fonts, and KaTeX's stylesheet and fonts, at fixed paths:
 *  `srcdoc` links `kit.css` and the maths chunk resolves `katex/` beside it. */
function kitFiles(): Plugin {
  return {
    name: "oculus:widget-kit-files",
    apply: "build",
    generateBundle() {
      const copy = (fileName: string, from: string) =>
        this.emitFile({ type: "asset", fileName, source: readFileSync(from) });
      copy("kit.css", path.join(kit, "kit.css"));
      for (const font of FONTS) copy(`fonts/${path.basename(font)}`, path.join(fontsource, font));
      copy("katex/katex.min.css", path.join(katex, "katex.min.css"));
      for (const font of readdirSync(path.join(katex, "fonts")))
        copy(`katex/fonts/${font}`, path.join(katex, "fonts", font));
    },
  };
}

export default defineConfig({
  plugins: [react(), kitFiles()],
  resolve: {
    // Mafs imports KaTeX at its top level; the kit typesets with math-core.
    alias: [{ find: /^katex$/, replacement: path.join(kit, "2d/katex.ts") }],
  },
  // Its own optimizer cache, so this build never touches the desktop dev server's.
  cacheDir: ".vite/widget-kit",
  // Relative, so every chunk and asset resolves against the URL it was served from.
  base: "./",
  publicDir: false,
  clearScreen: false,
  define: { "process.env.NODE_ENV": JSON.stringify("production") },
  build: {
    outDir: "dist/widget-kit",
    emptyOutDir: true,
    // The maths wasm stays a file the frame fetches, never a data: URL.
    assetsInlineLimit: 0,
    // No preload helper: it would add <link> tags the frame doesn't need.
    modulePreload: false,
    target: "es2022",
    // three is one ~750 KB chunk by design: a widget that imports it needs all of it.
    chunkSizeWarningLimit: 1000,
    rollupOptions: {
      input: {
        react: path.join(kit, "react.ts"),
        "jsx-runtime": path.join(kit, "jsx-runtime.ts"),
        ui: path.join(kit, "ui/index.ts"),
        state: path.join(kit, "state/index.ts"),
        "2d": path.join(kit, "2d/index.ts"),
        math: path.join(kit, "math/index.ts"),
        "3d": path.join(kit, "3d/index.ts"),
        three: path.join(kit, "three.ts"),
        fiber: path.join(kit, "fiber.ts"),
        ask: path.join(kit, "ask/index.ts"),
        runtime: path.join(kit, "runtime/index.tsx"),
      },
      // Keep every entry's exports: nothing in the bundle imports them, the import map does.
      preserveEntrySignatures: "strict",
      output: {
        format: "es",
        entryFileNames: "[name].js",
        chunkFileNames: "chunks/[name]-[hash].js",
        assetFileNames: "assets/[name]-[hash][extname]",
      },
    },
  },
});

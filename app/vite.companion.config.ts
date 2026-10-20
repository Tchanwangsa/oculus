import { defineConfig, type Plugin } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import path from "node:path";

/**
 * The page a paired browser loads from the Mac's companion server
 * (docs/device-sharing.md): `companion.html` and `src/companion/`, built to
 * `dist/companion/` after the desktop bundle, which the server serves under `/c/`.
 * The canvas WebAssembly and the fonts are emitted as hashed files under
 * `dist/companion/assets/`, so the page loads nothing from outside `/c/`.
 */

/** The server answers `/c/` with `index.html`; the entry is `companion.html` beside `index.html`. */
function companionIndex(): Plugin {
  return {
    name: "oculus:companion-index",
    apply: "build",
    enforce: "post",
    generateBundle(_options, bundle) {
      const page = bundle["companion.html"];
      if (page) page.fileName = "index.html";
    },
  };
}

export default defineConfig({
  plugins: [react(), tailwindcss(), companionIndex()],
  // Its own optimizer cache, so `vite -c` here never re-optimizes the desktop dev server's deps.
  cacheDir: ".vite/companion",
  base: "/c/",
  // The desktop's public files (its icons) are not the page's.
  publicDir: false,
  resolve: {
    // Exact matches first: `@/lib/menu/editRouting` has a browser stand-in (see that file).
    alias: [
      { find: /^@\/lib\/menu\/editRouting$/, replacement: path.resolve(__dirname, "./src/companion/editRouting.ts") },
      { find: "@", replacement: path.resolve(__dirname, "./src") },
    ],
  },
  clearScreen: false,
  build: {
    outDir: "dist/companion",
    emptyOutDir: true,
    assetsDir: "assets",
    // Fonts and the wasm stay files the server serves with their own MIME, never data: URLs.
    assetsInlineLimit: 0,
    // React, Radix and the canvas in one chunk the page needs at once.
    chunkSizeWarningLimit: 700,
    rollupOptions: { input: path.resolve(__dirname, "companion.html") },
  },
  server: { port: 1431, strictPort: true },
  preview: { port: 1431, strictPort: true },
});

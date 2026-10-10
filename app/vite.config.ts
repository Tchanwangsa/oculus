import { defineConfig, type Plugin } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import path from "node:path";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

/** Editor shadow mode is dev-only (docs/editor-core.md). Its loader's import
 *  of the wasm glue is dead code in a release build, but Rollup loads the glue
 *  anyway, and the glue's `new URL(wasm, import.meta.url)` would ship the wasm. */
function noEditorShadowInBuild(): Plugin {
  return {
    name: "oculus:no-editor-shadow-in-build",
    apply: "build",
    enforce: "pre",
    load: (id) => (id.includes("/editor/shadow/pkg/") ? "export {};" : null),
  };
}

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [react(), tailwindcss(), noEditorShadowInBuild()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));

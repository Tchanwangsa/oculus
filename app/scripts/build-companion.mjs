// Build the device-sharing companion page into dist/companion/ (docs/frontend.md),
// skipped when it is newer than every input. `tauri dev` serves the desktop
// page from Vite but the companion server reads the built page from disk, so
// the dev preflight runs this. Tailwind scans all of src/, so all of it counts.
import { execFileSync } from 'node:child_process';
import { readdirSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { app } from './runtime.mjs';

const mtime = p => { try { return statSync(p).mtimeMs; } catch { return 0; } };
const entries = dir => { try { return readdirSync(dir, { withFileTypes: true }); } catch { return []; } };
const newest = dir => Math.max(mtime(dir), ...entries(dir).map(e => e.isDirectory() ? newest(join(dir, e.name)) : mtime(join(dir, e.name))));
const inputs = Math.max(
  newest(join(app, 'src')),
  newest(join(app, 'canvas-core', 'pkg')),
  ...['companion.html', 'vite.companion.config.ts', 'package.json', 'bun.lock', 'tsconfig.json'].map(f => mtime(join(app, f))),
  mtime(fileURLToPath(import.meta.url)),
);
const page = join(app, 'dist', 'companion', 'index.html');
if (mtime(page) >= inputs) {
  console.log('[companion] up to date');
  process.exit(0);
}
const started = Date.now();
try {
  execFileSync('bun', ['run', 'companion:build'], { cwd: app, stdio: ['ignore', 'ignore', 'inherit'] });
  console.log(`[companion] built in ${((Date.now() - started) / 1000).toFixed(1)}s`);
} catch (error) {
  console.error(`[companion] ${error.message}`);
  process.exit(1);
}

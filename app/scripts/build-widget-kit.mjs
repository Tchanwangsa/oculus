// Build the chat widgets' kit into dist/widget-kit/ (docs/widgets.md), skipped
// when it is newer than every input. `tauri dev` serves the desktop page from
// Vite, but the media server reads the kit from disk, so the dev preflight
// runs this.
import { execFileSync } from 'node:child_process';
import { readdirSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { app } from './runtime.mjs';

const mtime = p => { try { return statSync(p).mtimeMs; } catch { return 0; } };
const entries = dir => { try { return readdirSync(dir, { withFileTypes: true }); } catch { return []; } };
const newest = dir => Math.max(mtime(dir), ...entries(dir).map(e => e.isDirectory() ? newest(join(dir, e.name)) : mtime(join(dir, e.name))));
const inputs = Math.max(
  newest(join(app, 'widget-kit')),
  newest(join(app, 'math-core', 'pkg')),
  newest(join(app, 'src', 'styles', 'katex')),
  ...['vite.widget-kit.config.ts', 'package.json', 'bun.lock'].map(f => mtime(join(app, f))),
  mtime(fileURLToPath(import.meta.url)),
);
const built = join(app, 'dist', 'widget-kit', 'runtime.js');
if (mtime(built) >= inputs) {
  console.log('[widget-kit] up to date');
  process.exit(0);
}
const started = Date.now();
try {
  execFileSync('bun', ['run', 'widget-kit'], { cwd: app, stdio: ['ignore', 'ignore', 'inherit'] });
  console.log(`[widget-kit] built in ${((Date.now() - started) / 1000).toFixed(1)}s`);
} catch (error) {
  console.error(`[widget-kit] ${error.message}`);
  process.exit(1);
}

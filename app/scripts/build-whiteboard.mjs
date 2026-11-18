// Build the retained canvas spatial core. Generated glue stays in canvas-core/pkg.
import { execFileSync } from 'node:child_process';
import { existsSync, readdirSync, statSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { app } from './runtime.mjs';

const core = join(app, 'canvas-core');
const out = join(core, 'pkg');
const target = 'wasm32-unknown-unknown';
const targetDir = process.env.CARGO_TARGET_DIR ?? join(core, 'target');
const wasm = join(targetDir, target, 'release', 'oculus_canvas_core.wasm');
const outputs = ['oculus_canvas_core.js', 'oculus_canvas_core.d.ts', 'oculus_canvas_core_bg.wasm', 'oculus_canvas_core_bg.wasm.d.ts'];
const mtime = p => { try { return statSync(p).mtimeMs; } catch { return 0; } };
const newest = dir => Math.max(mtime(dir), ...readdirSync(dir, { withFileTypes: true }).map(e => e.isDirectory() ? newest(join(dir, e.name)) : mtime(join(dir, e.name))));
const inputs = Math.max(newest(join(core, 'src')), mtime(join(core, 'Cargo.toml')), mtime(join(core, 'Cargo.lock')), mtime(fileURLToPath(import.meta.url)));
if (outputs.every(f => mtime(join(out, f)) >= inputs)) {
  console.log('[canvas] up to date');
  process.exit(0);
}
const run = (cmd, args, opts = {}) => execFileSync(cmd, args, { encoding: 'utf8', ...opts });
try {
  if (!run('rustup', ['target', 'list', '--installed']).split(/\r?\n/).includes(target)) throw new Error(`Run rustup target add ${target}.`);
  const version = readFileSync(join(core, 'Cargo.toml'), 'utf8').match(/^wasm-bindgen\s*=\s*"=([^"]+)"/m)?.[1];
  if (run('wasm-bindgen', ['--version']).trim() !== `wasm-bindgen ${version}`) throw new Error(`Run cargo install wasm-bindgen-cli --version ${version} --locked.`);
  run('cargo', ['build', '--locked', '--release', '--target', target], { cwd: core, stdio: 'inherit' });
  if (!existsSync(wasm)) throw new Error(`Missing cargo output: ${wasm}`);
  run('wasm-bindgen', ['--target', 'web', '--out-dir', out, wasm], { stdio: 'inherit' });
  console.log(`[canvas] built ${(statSync(join(out, outputs[2])).size / 1024).toFixed(0)} KiB wasm`);
} catch (error) {
  console.error(`[canvas] ${error.message}`);
  process.exit(1);
}

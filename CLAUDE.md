# Oculus

`docs/index.md` is the map — read the page for your area before searching
source. This file is only the rules; the reasons behind them live in `docs/`.

## Commands

- **bun only**, never npm/yarn/pnpm/npx (`app/bun.lock` is the only lockfile;
  Tauri shells out to `bun run`). One-off CLIs: `bunx`.
- `cd app && bun run tauri dev` runs the app. Its preflight also builds the
  `oculus` CLI, because `tauri dev` alone doesn't; `bun run cli:dev` does it by
  hand.
- `bun run ffmpeg` fetches the native ffmpeg into the gitignored
  `app/src-tauri/binaries/`.
- Checks: `bun run build` (tsc + bundle), `cargo check` / `cargo test` in
  `app/src-tauri`.
- Run `cargo fmt` in `app/src-tauri` before committing Rust; CI fails on
  `cargo fmt --check`.

## Don't re-add

- A Python process or `uv` step. Native inference helpers are fine
  (`apple-speech`, `whisper-cli`); the local MinerU engine is the user's own
  server over HTTP, not ours. Rust comments citing
  `sidecar/*.py` point at commit `f875bb1`.
- A BYOK API layer (opencode is the API path), or automations/the Inbox (last
  at `d64dc11`).

## Rules

- No work in hidden WebViews — scraping lives in Rust. (`docs/architecture.md`)
- Canvas auth is the session cookie; API tokens are disabled by the
  university. (`docs/auth.md`)
- Parses and embeds take minutes. Don't layer timeouts on top. Never add a
  fallback between parse engines, and a failed parse must surface.
  (`docs/parsing.md`, `docs/retrieval.md`)
- Nothing in Settings may make a billed call — no per-model probes.
  (`docs/harness.md`)
- Retrieval embeds page images, not text. (`docs/retrieval.md`)
- UI work: read `docs/ui.md` first — design tokens, the
  primitives' sizes, no toasts, no native date inputs, page zoom, and the
  WebKit drag traps. Screenshot the running app after UI changes
  (`docs/development.md`).

## Docs and comments

- Docs ship in the same commit as the code and describe it as it is now — no
  history. Removing or replacing a feature means removing every mention of it
  (`write-docs`). If a doc contradicts the code, trust the code and fix the
  doc. No per-directory `CLAUDE.md`.
- No banner comments (`// ── x ──`, `// --- x ---`). A file that needs section
  dividers is several files: split it into a folder module, one file per section.
- Comments say what the code does and why — never its history (that goes in
  the commit). A "why" is three lines at most; a module header fifteen; longer
  belongs in `docs/`. State a fact once and point to it elsewhere.

## Git

- Branch `master`. Commit messages match `git log --oneline`
  (`feat(scope):`, `fix(scope):`, `refactor(scope):`).
- Never commit `data/`, `*.db` or `app/src-tauri/binaries/`.

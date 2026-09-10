# Development

## Prerequisites

- **bun** (never npm/yarn/pnpm — see root `CLAUDE.md`)
- Rust toolchain (stable, via rustup)
- **uv** for the sidecar's Python environment
- macOS is the primary target (keep-alive, screenshots, and the WebView
  behaviour notes are macOS-specific)

## First-time setup

```sh
cd app
bun install
bun run ffmpeg        # fetches the ffmpeg binary into src-tauri/binaries/
cd ../sidecar
uv sync               # creates .venv (~1.2GB — that is the floor, mostly torch)
```

## Running

```sh
cd app
bun run tauri dev     # full desktop app (spawns the sidecar itself)
bun run dev           # vite only, browser — no Tauri APIs, limited use
bun run tauri build   # release build
bun run cli           # build the headless `oculus` binary
bun run cli:install   # + symlink into ~/.local/bin
bun run stage-cli     # build it and stage it as a sidecar for the bundle
bun run docs:cli      # regenerate docs/cli-reference.md from the binary's help
```

`tauri build` runs `stage-cli` for you (it is in `beforeBuildCommand`): the
`oculus` CLI ships inside the app because the macOS keep-alive LaunchAgent runs
it. `bun run cli` is the plain build for working on the CLI itself; the two
share the same compiled binary. See [auth.md](./auth.md) for why the staging
step writes a placeholder on a cold build.

`docs:cli` follows `stage-cli` in the same hook, which is the only reason it is
cheap: the release binary is already built and current, so regenerating
[cli-reference.md](./cli-reference.md) is one process launch. It is not on
`beforeDevCommand` — `tauri dev` never builds the CLI, so hooking it there
would put a release build in front of every dev start. Run it by hand after
changing the CLI if you want the repo copy current before the next bundle.

The sidecar can also be run by hand (`cd sidecar && uv run main.py`) for
debugging — keep Oculus off port 9547 via the env override documented at the
top of `app/src-tauri/src/sidecar.rs`, and remember stdout is block-buffered
(`docs/sidecar.md`).

## Checks

- Frontend type-check + bundle: `cd app && bun run build` (runs `tsc`).
- Rust: `cargo check` in `app/src-tauri` (or just let `tauri dev` rebuild).
- Retrieval smoke test: `app/src-tauri/src/bin/retrieval_smoke.rs`.
- Sidecar regressions: `cd sidecar && uv run python -m unittest discover`.
  The balloon test allocates a small synthetic worker and verifies kill and
  restart without loading models or contacting MinerU.
- Real quality regression: `cd sidecar && uv run python benchmark_quality.py
  /path/to/deck.pdf --memory-cap 8192`. It copies the source to a temporary
  directory, performs fast + local quality, and reports whole-tree peak and
  kills. It needs cached/downloadable MinerU weights and can take minutes.
- After UI changes, screenshot the running app (root `CLAUDE.md` has the
  incantation) — the WebView is where layout bugs actually show.

## How it connects

The Rust supervisor passes the persisted parse settings to the sidecar at
spawn. A standalone sidecar can use these environment variables instead:

| Variable | Meaning |
| --- | --- |
| `OCULUS_SIDECAR_EXTERNAL` | Tell the app not to spawn/reclaim the sidecar |
| `OCULUS_SIDECAR_MEMORY_CAP_MB` | Whole-tree cap, default 8192, floor 5120 |
| `OCULUS_MINERU_BACKEND` | `local` (default), `cloud`, or `auto` |
| `OCULUS_DATA_DIR` | Directory containing the persistent cloud usage ledger |
| `OCULUS_MINERU_WINDOW_PAGES` | Lower the local rendered-page window; hard maximum 8 |
| `OCULUS_MINERU_CHUNK_PAGES` | Lower the outer progress chunk; maximum 64 |
| `OCULUS_MEMORY_SAMPLE_SECONDS` | Watchdog cadence, default 1 second (profiling/tests) |

Changing the cap live uses `/limits`, so it can terminate a worker if its
current footprint exceeds the new budget. Cloud tokens are **not** environment
settings: Rust retrieves them from the keychain for the loopback parse request.
Cloud development does not require enabling uploads; protocol/routing tests
mock external calls. See [sidecar.md](./sidecar.md) for privacy and API limits.

## Gotchas

- `tauri dev` rebuilds SIGTERM the app in a way that bypasses Tauri's Exit
  event; the sidecar supervisor installs its own handlers, so orphaned
  uvicorns should not happen — if one does, the next launch reclaims the
  port.
- User data lives in `~/Library/Application Support/com.tchan.oculus`
  (cookie, `oculus.db`, `courses/`, `lectures/`). Deleting it is a full
  reset, including auth.
- `data/`, `*.db`, `sidecar/.venv/`, `app/src-tauri/binaries/` are
  gitignored; never commit them.

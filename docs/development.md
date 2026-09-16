# Development

## Prerequisites

- **bun** (never npm/yarn/pnpm — see root `CLAUDE.md`)
- Rust toolchain (stable, via rustup)
- macOS is the primary target (keep-alive, screenshots, and the WebView
  behaviour notes are macOS-specific)

## First-time setup

```sh
cd app
bun install
bun run ffmpeg        # fetches the ffmpeg binary into src-tauri/binaries/
bun run pdfium        # fetches libpdfium into src-tauri/binaries/
```

**There is no Python step any more**, and no `sidecar/` directory: it left the
tree in `f875bb1`, which is the commit the separate local-server repo forks
from. If you have an orphaned `sidecar/.venv` from an older checkout it is
1.2 GB of nothing — delete it.

Both fetch steps also run from `beforeDevCommand` / `beforeBuildCommand`, so
`bun run tauri dev` sets them up on its own; they are listed here because a
`cargo test` or a `bun run cli` on a fresh checkout does not go through Tauri.

`libpdfium` is the page rasterizer behind `app/src-tauri/src/embed/raster.rs` —
a native C++ library with no crates.io source, so `app/scripts/fetch-pdfium.mjs`
downloads a prebuilt one from bblanchon/pdfium-binaries. Its release tag is
pinned to the Chromium revision `pdfium-render`'s feature flag binds against: a
lib from another revision fails at *bind* time, not at compile time, so the two
move together. Neither binary is committed — `app/src-tauri/binaries/` is
gitignored. At runtime the library is found relative to the executable
(`Contents/Frameworks/` in the bundled `.app`, an ancestor `binaries/` in dev),
and `OCULUS_PDFIUM_LIB` overrides that with an explicit path.

## Running

```sh
cd app
bun run tauri dev     # full desktop app
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

## Checks

- Frontend type-check + bundle: `cd app && bun run build` (runs `tsc`).
- Rust: `cargo check` in `app/src-tauri` (or just let `tauri dev` rebuild).
- Retrieval smoke test: `app/src-tauri/src/bin/retrieval_smoke.rs`.
- Parse regressions: `cargo test` in `app/src-tauri`. The differential tests in
  `parse/mineru/render.rs` pin the Python renderer's own output and are now the
  only record of what it did; none of them touch the network.
- Real parse regression: the golden fixtures in `data/parse-fixtures/`
  (gitignored) — see [parsing.md](./parsing.md#debugging).
- After UI changes, screenshot the running app (root `CLAUDE.md` has the
  incantation) — the WebView is where layout bugs actually show.

## Environment overrides

| Variable | Meaning |
| --- | --- |
| `OCULUS_DATA_DIR` | The data directory, including both cloud usage ledgers |
| `OCULUS_PDFIUM_LIB` | Explicit path to `libpdfium`, instead of the search relative to the executable |

Which parser and which embedder run is a **setting, not an environment
variable** — the `parse` and `embed` rows in SQLite, written from Settings →
Library. Neither cloud's credential is an environment variable either: both
come from the keychain and are handed straight to an in-process client, so
neither crosses a socket on this machine. Working on either protocol needs no
real key — the client tests run against a fake server. See
[parsing.md](./parsing.md) and [retrieval.md](./retrieval.md) for privacy and
API limits.

## Gotchas

- `tauri dev` rebuilds SIGTERM the app in a way that bypasses Tauri's Exit
  event. Nothing the app spawns outlives it any more — the one long-lived
  child process was the sidecar — but a CLI-agent subprocess mid-turn is the
  case to watch (see [harness.md](./harness.md)).
- User data lives in `~/Library/Application Support/com.tchan.oculus`
  (cookie, `oculus.db`, `courses/`, `lectures/`). Deleting it is a full
  reset, including auth.
- `data/`, `*.db` and `app/src-tauri/binaries/` are gitignored; never commit
  them.

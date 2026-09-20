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
bun run predev        # deps, ffmpeg, libpdfium and the `oculus` CLI
```

`predev` is the whole preflight and it is idempotent — run it any time. It is
also bun's lifecycle name for `dev`, so `bun run dev` runs it first and so does
`bun run tauri dev`, whose `beforeDevCommand` is now just
`OCULUS_CLI_WATCH=1 bun run dev`. `OCULUS_SKIP_PREDEV=1` skips it when you only
want vite.

**There is no Python step any more**, and no `sidecar/` directory: it left the
tree in `f875bb1`, which is the commit the separate local-server repo forks
from. If you have an orphaned `sidecar/.venv` from an older checkout it is
1.2 GB of nothing — delete it.

The two fetch steps (`bun run ffmpeg`, `bun run pdfium`) are still there to run
on their own, and `beforeBuildCommand` calls them directly; they are worth
knowing about because a `cargo test` or a `bun run cli` on a fresh checkout
does not go through Tauri or through `predev`.

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
bun run predev        # the dev preflight, by hand
bun run cli           # build the headless `oculus` binary (release)
bun run cli:dev       # the same binary in the debug profile
bun run cli:install   # release build + symlink into ~/.local/bin
bun run stage-cli     # build it and stage it as a sidecar for the bundle
bun run docs:cli      # regenerate docs/cli-reference.md from the binary's help
```

### The dev CLI

The `oculus` CLI is a second binary in the same crate, and for a long time
nothing in the dev path built it. `tauri dev` issues a bare `cargo run`, which
builds `app` and no other bin target; `bun run cli` builds the *release* one by
hand. So `target/debug/oculus` was whatever a stray `cargo test` last left
there — measured once at ten days and several subcommands out of date — while
being the sibling of the running app and therefore the first `oculus` on a
coding agent's PATH (`child_env` in
`app/src-tauri/src/harness/discover.rs`). The agent ran `oculus project create`
and got `unrecognized subcommand` from a binary that looked entirely
legitimate.

Three things close that, and they are worth knowing separately because they
cover different windows:

- **`app/scripts/predev.mjs`** builds it at the start of every dev session.
  Debug, not release: the dev app runs out of `target/debug`, so those
  artifacts are already warm and the build is ~25s after a Rust change and ~3s
  when nothing moved. A release CLI in front of every dev start would be a
  minute spent optimising a binary the session does not use.
- **`app/scripts/watch-cli.mjs`** keeps it current *through* a session.
  `tauri dev` re-runs cargo and relaunches the app on a Rust change but never
  re-runs `beforeDevCommand`, so without this the CLI drifts back to the
  vintage of whenever the session began. It deliberately loses the race: its
  debounce is longer than tauri's, so the app's build takes cargo's package
  lock first and the dev loop keeps the latency it had. Started detached by
  `predev` when `OCULUS_CLI_WATCH=1`, and it ends itself when the vite port
  stops answering.
- **`discover::warn_if_stale`** says so in the dev terminal if a CLI under
  `target/` is older than the `.rs` files beside it. It should never fire; it
  exists because the failure is invisible from the agent's end, where a stale
  binary runs, answers `--version`, and rejects a subcommand it has never heard
  of. A binary from anywhere else — a bundle's sidecar, `~/.local/bin`, the
  PATH — has no sources to be behind, so it is silent by construction.

Both `predev` and the watcher delete the binary before building. Cargo reports
"Finished" while leaving the previous one in place when it decides the uplift
from `target/debug/deps` is unnecessary, which is the exact failure the
preflight exists to prevent. The cost is a few seconds after each edit with no
`oculus` at that path at all — which is the right way round, and is why
`oculus_cli` ranks the candidates it finds instead of trusting one.

`tauri build` runs `stage-cli` for you (it is in `beforeBuildCommand`): the
`oculus` CLI ships inside the app because the macOS keep-alive LaunchAgent runs
it. `bun run cli` is the plain build for working on the CLI itself; the two
share the same compiled binary. See [auth.md](./auth.md) for why the staging
step writes a placeholder on a cold build.

`docs:cli` follows `stage-cli` in the same hook, which is the only reason it is
cheap: the release binary is already built and current, so regenerating
[cli-reference.md](./cli-reference.md) is one process launch. `predev` runs it
too, against the debug binary it has just built, and the generator only writes
when the help actually changed — so the reference moves in the same commit as
the CLI rather than waiting for the next bundle.

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

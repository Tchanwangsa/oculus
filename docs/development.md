# Development

How to build, run and check the app and the `oculus` CLI. macOS is the primary
target; you need bun (never npm — see the root `CLAUDE.md`) and stable Rust.

## Where

| Piece | Location |
| --- | --- |
| Scripts (`predev`, `cli`, `cli:dev`, `cli:install`, `stage-cli`, `docs:cli`, …) | `app/package.json` |
| `beforeDevCommand`, `beforeBuildCommand`, `externalBin` | `app/src-tauri/tauri.conf.json` |
| Cargo CLI build, target paths and host detection | `app/scripts/runtime.mjs`, `app/scripts/build-cli.mjs` |
| Shared cached native download and installation | `app/scripts/native-binary.mjs` |
| The dev preflight | `app/scripts/predev.mjs` |
| Keeps the dev CLI current through a session | `app/scripts/watch-cli.mjs` |
| Native binary fetchers | `app/scripts/fetch-pdfium.mjs`, `app/scripts/fetch-ffmpeg.mjs` |
| Stages the CLI into the bundle | `app/scripts/stage-cli.mjs` |
| Regenerates `docs/cli-reference.md` | `app/scripts/gen-cli-docs.mjs` |
| How an agent thread finds `oculus` | `app/src-tauri/src/harness/discover.rs` |

## Commands

```sh
cd app
bun install
bun run tauri dev     # the desktop app (runs the preflight first)
bun run dev           # vite only, in a browser — no Tauri APIs
bun run tauri build   # release bundle
bun run predev        # the dev preflight, by hand
bun run cli           # release `oculus`
bun run cli:dev       # debug `oculus` — the one the dev app's agents run
bun run cli:install   # release build, symlink into ~/.local/bin, then `oculus docs`
bun run docs:cli      # regenerate docs/cli-reference.md from the binary's help
bun run pdfium        # fetch libpdfium into app/src-tauri/binaries/
bun run ffmpeg        # fetch ffmpeg into app/src-tauri/binaries/
```

## `predev` is the whole preflight, and it is idempotent

It is bun's lifecycle hook for `dev`, and `beforeDevCommand` is
`OCULUS_CLI_WATCH=1 bun run dev`, so every `tauri dev` runs it. In order it
runs `bun install`, fetches ffmpeg and pdfium, builds the debug `oculus`,
regenerates [cli-reference.md](./cli-reference.md) (written only when the help
changed), and runs `oculus docs` to refresh the library's agent docs. The last
two are non-fatal: a machine whose app has never run has no library to fill.
`OCULUS_SKIP_PREDEV=1` skips it all for a vite-only session.

The scripts share target detection and artifact paths through `app/scripts/runtime.mjs`;
both native fetchers reject failed HTTP responses and truncated downloads before
installing anything. Pdfium extracts into a private temporary directory per run.

`cargo test` and `bun run cli` go through neither Tauri nor `predev`, so on a
fresh checkout run `bun run pdfium` and `bun run ffmpeg` yourself.

## The dev CLI is built by the preflight, not by `tauri dev`

`tauri dev` issues a bare `cargo run`, which builds the `app` bin and no other.
Yet `target/debug/oculus` sits beside the running app, so `child_env` in
`app/src-tauri/src/harness/discover.rs` puts it first on every agent thread's
PATH. Three pieces keep it current:

- `app/scripts/predev.mjs` builds it at each dev start, in the debug profile
  because those artifacts are already warm from the app build.
- `app/scripts/watch-cli.mjs` rebuilds it on Rust changes through the session,
  since `tauri dev` never re-runs `beforeDevCommand`. Its debounce is longer
  than tauri's, so the app build takes cargo's lock first. It exits when the
  vite port stops answering.
- `discover::warn_if_stale` logs in the dev terminal when a CLI under
  `target/` is older than the sources beside it.

`runtime.mjs` owns CLI paths and cargo arguments. The preflight, bundle
staging, watcher and `bun run cli`/`cli:dev` **delete the binary before
building**: cargo can report "Finished" while leaving the previous binary in
place. `oculus_cli` ranks every candidate it finds by mtime rather than
trusting one, which covers the seconds when the dev path has no binary at all.

`tauri build` runs `stage-cli` then `docs:cli` from `beforeBuildCommand`: the
CLI ships inside the bundle because the keep-alive LaunchAgent runs it
([auth.md](./auth.md)), and the reference is regenerated at the one moment a
current release binary is guaranteed to exist.

## Template edits reach the library only through `oculus docs`

The agent-facing files — `AGENTS.md`, the skills, `OCULUS-CLI.md`, `TASTE.md`'s
guidance, the `MEMORY.md` indexes — live in the data directory, written from
`app/src-tauri/templates/` by `oculus docs` (and by every sync). `predev` runs
it, so a template edit lands at the next dev start. `HARNESS.template.md` is the
exception: it is `include_str!`'d into the app, so it changes with the Rust
rebuild. Which files are overwritten versus merged is in
[cli.md](./cli.md#oculus-docs-writes-the-agents-folder).

## `libpdfium` is pinned to `pdfium-render`'s Chromium revision

It rasterizes pages for embedding (`app/src-tauri/src/embed/raster.rs`) and has
no crates.io source, so `app/scripts/fetch-pdfium.mjs` downloads a prebuilt one.
Its release tag must match the revision `pdfium-render`'s feature flag binds
against: a mismatch fails at *bind* time, not compile time, so bump both
together. At runtime it is found beside the executable (`Contents/Frameworks/`
in the bundle, an ancestor `binaries/` in dev); `OCULUS_PDFIUM_LIB` overrides
the path.

## A UI change is verified by screenshot, not by `tsc`

- Frontend type-check and bundle: `cd app && bun run build`; editor, shared
  frontend logic and offline script regressions: `bun run test`.
- Rust: `cargo check` / `cargo test` in `app/src-tauri`. None of the tests
  touch the network: the cloud clients run against a fake server, and the
  renderer tests in `app/src-tauri/src/parse/mineru/render.rs` pin output
  against the renderer it was ported from.
- Real parse regressions: the gitignored golden fixtures in
  `data/parse-fixtures/` ([parsing.md](./parsing.md)).
- Retrieval smoke test: `app/src-tauri/src/bin/retrieval_smoke.rs`.
- After UI changes, screenshot the running app — layout bugs show only in the
  WebView: `screencapture -x -o -l<windowid>`, where the dev window's owner is
  "app".

The parser and embedder are settings (Settings → Library) and their keys live
in the keychain — neither is an environment variable.

## Gotchas

- A cargo build can leave a stale `oculus` and say "Finished" — delete the binary first.
- `tauri dev` alone never builds `oculus` — use `predev` or `bun run cli:dev`, or agents run an old CLI.
- Anything under `app/src-tauri/` changing, templates included, rebuilds and relaunches the dev app.
- A dev rebuild SIGTERMs the app past Tauri's Exit event, so agent subprocesses can outlive it ([harness.md](./harness.md)).
- A pdfium from the wrong Chromium revision builds fine and fails at bind time.
- Deleting `~/Library/Application Support/com.tchan.oculus` is a full reset, sign-in included.
- `data/`, `*.db` and `app/src-tauri/binaries/` are gitignored — never commit them.

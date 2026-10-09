# Development

How to build, run and check the app and the `oculus` CLI. macOS is the primary
target; you need bun (never npm — see the root `CLAUDE.md`) and stable Rust.

## Where

| Piece | Location |
| --- | --- |
| Scripts (`predev`, `cli`, `cli:dev`, `cli:install`, `stage-cli`, `docs:cli`, …) | `app/package.json` |
| `beforeDevCommand`, `beforeBuildCommand`, `externalBin` | `app/src-tauri/tauri.conf.json`; macOS's `externalBin` in `app/src-tauri/tauri.macos.conf.json` |
| Cargo CLI build, target paths and host detection | `app/scripts/runtime.mjs`, `app/scripts/build-cli.mjs` |
| Shared cached native download and installation | `app/scripts/native-binary.mjs` |
| The dev preflight | `app/scripts/predev.mjs` |
| Keeps the dev CLI current through a session | `app/scripts/watch-cli.mjs` |
| Native binary fetchers | `app/scripts/fetch-pdfium.mjs`, `app/scripts/fetch-ffmpeg.mjs` |
| Compiles the on-device speech helper | `app/scripts/build-speech.mjs`, `app/src-tauri/speech/main.swift` |
| Builds whisper.cpp's `whisper-cli` from a pinned release | `app/scripts/build-whisper.mjs` |
| Builds, signs and (main checkout only) installs `oculus-keyd` | `app/scripts/build-keyd.mjs`, `app/keyd/.cargo/config.toml` |
| Keeps keyd's OS calls inside its adapters | `app/scripts/keyd-seams.test.mjs` |
| Stages the CLI into the bundle | `app/scripts/stage-cli.mjs` |
| Regenerates `docs/cli-reference.md` | `app/scripts/gen-cli-docs.mjs` |
| How an agent thread finds `oculus` | `app/src-tauri/src/harness/discover.rs` |
| CI checks and the release bundle | `.github/workflows/ci.yml`, `.github/workflows/release.yml` |

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
bun run docs:cli      # rebuild the release CLI, regenerate docs/cli-reference.md from its help
bun run pdfium        # fetch libpdfium into app/src-tauri/binaries/
bun run ffmpeg        # fetch ffmpeg into app/src-tauri/binaries/
bun run speech        # compile the on-device speech helper there (macOS)
bun run whisper       # build whisper.cpp's whisper-cli there (macOS, needs cmake)
bun run keyd          # build and sign oculus-keyd; install it from the main checkout
```

## `predev` is the whole preflight, and it is idempotent

It is bun's lifecycle hook for `dev`, and `beforeDevCommand` is
`OCULUS_CLI_WATCH=1 bun run dev`, so every `tauri dev` runs it. In order it
runs `bun install`, fetches ffmpeg and pdfium, compiles the speech helper and
`whisper-cli`, builds the debug `oculus`, builds and signs `oculus-keyd`
(failing like the CLI build does, and installing only as
[below](#oculus-keyd-is-built-apart-so-its-signature-only-changes-with-its-source)),
regenerates [cli-reference.md](./cli-reference.md) (written only when the help
changed), and runs `oculus docs` to refresh the library's agent docs. The last
two are non-fatal: a machine whose app has never run has no library to fill.
`OCULUS_SKIP_PREDEV=1` skips it all for a vite-only session.

The scripts share target detection and artifact paths through `app/scripts/runtime.mjs`;
both native fetchers reject failed HTTP responses and truncated downloads before
installing anything. Pdfium extracts into a private temporary directory per run.

`cargo test` and `bun run cli` go through neither Tauri nor `predev`, so on a
fresh checkout run `bun run pdfium`, `bun run ffmpeg` and (on macOS)
`bun run speech` and `bun run whisper` yourself.

## The speech helper is compiled, not fetched

`apple-speech` wraps macOS 26's on-device recogniser for transcription
([viewers.md](./viewers.md#on-device-speech-a-swift-helper)). `bun run speech`
compiles `app/src-tauri/speech/main.swift` with `swiftc` into
`binaries/apple-speech-<host triple>`, skipping when the binary is newer than
the source; it needs Xcode or its command-line tools with the macOS 26 SDK,
and fails loudly without them. Elsewhere it logs and does nothing.

It is an `externalBin` on macOS only, through `tauri.macos.conf.json`, which
Tauri merges over `tauri.conf.json` as a JSON Merge Patch — arrays are
replaced, so that file lists ffmpeg and oculus again. tauri-build reads the
merged list, so on a Mac `cargo check` refuses to run until the helper exists,
exactly as for ffmpeg.

## whisper-cli is built from a pinned release

`bun run whisper` downloads the whisper.cpp release tarball named by `VERSION`
in `app/scripts/build-whisper.mjs`, refuses it unless it matches `SHA256`, and
builds only the `whisper-cli` target with cmake (`brew install cmake`; CI
runners have it) into `binaries/whisper-cli-<host triple>`. Source and build
tree are cached in `app/node_modules/.cache/whisper.cpp/`, outside the
`src-tauri` tree that `tauri dev` watches. It skips while the installed binary's
`--version` names the pinned release; `--force` rebuilds. To upgrade, change
`VERSION` and `SHA256` together.

The build is static with Metal's shaders embedded, generic rather than tuned
to the building CPU, and without OpenMP, so the binary links only system
libraries and frameworks; the script checks `otool -L` and fails otherwise.
Its floor is macOS 13.3, upstream's own. Like `apple-speech` it is a macOS-only
`externalBin` through `tauri.macos.conf.json`.

## The dev CLI is built by the preflight, not by `tauri dev`

`tauri dev` issues a bare `cargo run`, which builds the `app` bin and no other.
Yet `target/debug/oculus` sits beside the running app, so `child_env` in
`app/src-tauri/src/harness/discover.rs` puts it first on every agent thread's
PATH. Four pieces keep it current:

- `app/scripts/predev.mjs` builds it at each dev start, in the debug profile
  because those artifacts are already warm from the app build.
- `app/scripts/watch-cli.mjs` rebuilds it on Rust changes through the session,
  since `tauri dev` never re-runs `beforeDevCommand`. Its debounce is longer
  than tauri's, so the app build takes cargo's lock first. It exits when the
  vite port (probed on `localhost`, which is `::1` on macOS) stops answering.
- Both, and `bun run cli`/`cli:dev`, copy each build over the
  `binaries/oculus-<triple>` sidecar. tauri-build copies every `externalBin`
  into `target/<profile>/` whenever the app's build script runs, so `tauri
  dev`, `cargo test` or any other build of the crate replaces
  `target/<profile>/oculus` with whatever the sidecar holds.
- `discover::warn_if_stale` logs in the dev terminal when a CLI under
  `target/` is older than the sources beside it.

`runtime.mjs` owns CLI paths, cargo arguments and the sidecar copy. The
preflight, bundle staging, watcher, `bun run cli`/`cli:dev` and `docs:cli`
**delete the binary before building**, so a build that leaves nothing behind
fails loudly instead of passing on an old file. `oculus_cli` ranks every candidate it finds
by mtime rather than trusting one, which covers the seconds when the dev path
has no binary at all.

`tauri build` runs `stage-cli` then `docs:cli` from `beforeBuildCommand`: the
CLI ships inside the bundle because the keep-alive LaunchAgent runs it
([auth.md](./auth.md)), and the reference is regenerated at the one moment a
current release binary is guaranteed to exist.

## CI proves a fresh checkout builds; releases are cut by hand

`ci.yml` runs on every push to `master` and every pull request, on a macOS
runner with nothing cached but crates: `cargo fmt --check` (the app, keyd
and keyd core), `bun run test`, `bun run build`, the two native fetches, the
speech helper and `whisper-cli`, `stage-cli`, `cargo test --release --locked`,
keyd core's tests with every feature and keyd's with and without `dev`, then
`docs:cli` with a `git diff --exit-code` so a CLI change that skipped the
reference fails. A second job, `keyd-linux`, checks and tests keyd core and
checks keyd on Ubuntu, where only the unsupported adapter exists
([below](#keyds-os-code-lives-in-one-adapter)); keyd's own tests bind a socket
through the macOS adapter, so they run only in the first job. `stage-cli`
comes before any cargo call because tauri-build refuses to compile until every `externalBin` exists, and it is what writes the
placeholder sidecar on a clean tree. The release profile is shared with that
CLI build, so the tests reuse its artifacts. sccache is installed because
`app/src-tauri/.cargo/config.toml` makes it rustc's wrapper.

`release.yml` is `workflow_dispatch` only. To cut a release, bump `version` in
`app/src-tauri/tauri.conf.json` and `app/src-tauri/Cargo.toml` together, push,
then run **Release** from the Actions tab. `tauri-apps/tauri-action` runs
`tauri build` (so `beforeBuildCommand` fetches natives, compiles the speech
helper and stages the CLI),
creates tag `v<version>` on that commit, and attaches the `.dmg` and `.app` to
a draft prerelease; the same files are kept as workflow artifacts. Publishing
the draft is a manual step on GitHub.

- The matrix has one entry, Apple Silicon. Another platform is one more
  `include` with its runner and `args` (`--target …`); the sidecars are named by
  `hostTriple()`, so build on the runner whose triple you want.
- The bundle is signed ad-hoc (`APPLE_SIGNING_IDENTITY: "-"`). A downloaded copy
  is quarantined — `xattr -dr com.apple.quarantine /Applications/Oculus.app` —
  and, having no team identifier, re-prompts for keychain items after every
  update. A Developer ID certificate plus notarisation (the `APPLE_*` secrets
  tauri-action reads) removes both.
- The installed app and `tauri dev` share the identifier `com.tchan.oculus`, so
  they open the same library, database and keychain items. Never run both at
  once: each re-points the keep-alive LaunchAgent at its own `oculus` at
  startup.

## `oculus-keyd` is built apart so its signature only changes with its source

macOS asks once per *binary* that reads a keychain item, and an ad-hoc
signature changes whenever the bytes do. So keyd
([architecture.md](./architecture.md#oculus-keyd-is-the-only-process-meant-to-read-its-key))
is built to give the same bytes for the same source:

- **Its own crate, never a workspace member.** `app/keyd/` has its own
  `Cargo.lock`, target dir and exact-pinned dependencies, so a bump in the
  app's lock never relinks it. Its logic is `app/keyd/core/`
  (`oculus-keyd-core`), which sits inside keyd's root (and is excluded from
  it) because cargo hashes a path dependency outside the root by its absolute
  path, which gives each checkout different bytes. The app links core with
  only its `client` feature (the client, the registrar and the Okta sign-in
  flow, which runs in-process while keyd is absent); keyd links it
  with `server` (the vault, the ops, `forward`, the server loop). So an app
  edit never touches keyd, and a core edit is a keyd source change.
- **Reproducible flags.** `-Wl,-S` in `app/keyd/.cargo/config.toml` keeps the
  object paths out of LC_UUID; `build-keyd.mjs` adds `--remap-path-prefix` for
  the crate, `~/.cargo` and `~/.rustup` through `--config`, which cargo joins
  onto that file's list. Cargo reads that file only when run from
  `app/keyd`, so build from there.
- **Signed as a fresh copy** — `codesign -s - -f -o runtime -i
  com.tchan.oculus.keyd` on a temp file renamed into `target/signed/`.
  Re-signing a file that has already run leaves the kernel's cached signature
  stale, and `-i` keeps the cdhash independent of the file name.
- **Installed by source hash, not bytes.** `build.rs` hashes keyd's and
  core's sources and manifests, keyd's `Cargo.lock` and its cargo config;
  `oculus-keyd source-hash` prints it and `oculus keyd install` writes it to
  `bin/oculus-keyd.stamp` in the data dir. The script installs only from the
  main checkout (`git rev-parse --git-dir` equals `--git-common-dir`), so a
  worktree build never takes over the LaunchAgent, and then through `oculus
  keyd install --if-changed`, which does nothing when the stamp already
  matches and the agent already runs that program. `oculus keyd install
  --from <path>` works anywhere.
- **`--features dev`** is what `bun run keyd` builds: it admits any same-user
  caller, because a dev keyd has no bundle to check callers against. Test
  hooks (`serve-local`, a data-dir override, an injected key that also turns
  off reading old keychain items, a loopback origin for Voyage, MinerU or
  Groq through `OCULUS_KEYD_<NAME>_ORIGIN`) exist only in debug builds.
- Checks: `cargo test` in `app/keyd` with and without `--features dev`, and
  `cargo test --all-features` in `app/keyd/core`. No test touches launchd or
  the keychain; the Okta sign-in runs against a loopback Canvas and Okta.

## keyd's OS code lives in one adapter

keyd behaves the same on every OS; what differs is how. Each OS call — the
socket, launchd's activation, the caller's audit token and signature, the
keychain, the LaunchAgent, `flock` and file modes — sits under
`app/keyd/core/src/platform/`, whose `mod.rs` states the contract and picks
the adapter by `cfg(target_os)`. Everything else in core, keyd's `main`, and
the app's `credentials.rs`, `keyd.rs` and `bin/oculus/keyd.rs` stay OS-free.

- **The build step** is the adapter's too: `build-keyd.mjs` keeps one
  function per OS (`buildForMacos`, the reproducible build and the ad-hoc
  signature above) and builds nothing on an OS without one.
- **Core's tests run anywhere.** The server loop and the client are tested
  over an in-memory `Conn` (`platform/memory.rs`); the macOS adapter tests
  its socket, caller check, keychain errors and plist itself. The POSIX file
  helpers (`platform/unix.rs`) serve every Unix, so the vault works in
  Linux CI.
- **The strict seal check is paid on every connection.** Under the install
  policy `seal_check` re-verifies the whole bundle each time: 0.7 ms for a
  bundle of one small executable, 32 ms for a signed 80 MB one (Apple silicon),
  plus 0.2 ms to inspect the caller. `peer.rs`'s ignored test
  `what_the_caller_check_costs_per_connection` measures it
  (`cargo test --all-features -- --ignored --nocapture`);
  `KEYD_COST_BUNDLE=<signed .app>` times a real bundle too.
- **Enforced twice.** `keyd-seams.test.mjs` (in `bun run test`) fails on any
  `std::os::unix`, `libc::`, framework, `extern "C"`, `launchctl` or
  `Library/Application Support` outside `platform/` in those files, comments
  included; the `keyd-linux` CI job builds core and keyd against the
  unsupported adapter.
- **Adding an OS** is a `platform/<os>.rs` written to the contract, its
  build step in `build-keyd.mjs`, and a CI row. If anything outside
  `platform/` has to change, the seam is wrong: fix it in core first.

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
- Rust: `cargo fmt`, `cargo check` / `cargo test` in `app/src-tauri`, and
  the same in keyd's two crates
  ([above](#oculus-keyd-is-built-apart-so-its-signature-only-changes-with-its-source)). None of the tests
  touch the network: the cloud clients run against a fake server, and the
  renderer tests in `app/src-tauri/src/parse/mineru/render.rs` pin output
  against the renderer it was ported from.
- Real parse regressions: the gitignored golden fixtures in
  `data/parse-fixtures/` ([parsing.md](./parsing.md)).
- After UI changes, screenshot the running app — layout bugs show only in the
  WebView: `screencapture -x -o -l<windowid>`, where the dev window's owner is
  "app".

The parser and embedder are settings (Settings → Parsing and Settings →
Embeddings) and their keys live in `oculus-keyd`'s vault, or the keychain
without keyd — neither is an environment variable.

## Gotchas

- Any build of the crate copies the `binaries/` sidecar to `target/<profile>/oculus` — a stale sidecar means a stale CLI.
- `tauri dev` alone never builds `oculus` — use `predev` or `bun run cli:dev`, or agents run an old CLI.
- Anything under `app/src-tauri/` changing, templates included, rebuilds and relaunches the dev app.
- A dev rebuild SIGTERMs the app past Tauri's Exit event, so agent subprocesses can outlive it ([harness.md](./harness.md)).
- A pdfium from the wrong Chromium revision builds fine and fails at bind time.
- Adding `app/keyd` to a workspace, or building it outside `app/keyd`, changes its bytes and brings back the keychain prompt.
- An OS call outside `app/keyd/core/src/platform/` in keyd's or the app's keyd code fails `bun run test`, and on Linux CI fails the build.
- Deleting `~/Library/Application Support/com.tchan.oculus` is a full reset, sign-in included.
- `data/`, `*.db` and `app/src-tauri/binaries/` are gitignored — never commit them.

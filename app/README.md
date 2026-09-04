# Oculus — desktop app

Tauri 2 + React 19 + Vite + Tailwind v4.

## Package manager: bun

**This project uses [bun](https://bun.sh). Do not use npm, yarn, or pnpm.**
`bun.lock` is the only lockfile in the repo; competing lockfiles are
gitignored so the two can never drift apart. `src-tauri/tauri.conf.json`
shells out to `bun run` for both dev and release builds, so an npm-installed
`node_modules` will not be what Tauri actually ships.

```sh
bun install          # install dependencies
bun run dev          # vite dev server (browser only)
bun run tauri dev    # full desktop app
bun run tauri build  # release build
```

Run one-off CLI tools with `bunx`, not `npx` — e.g. `bunx shadcn@latest add <component>`.

## `oculus` — the command line

A second binary in `src-tauri` that drives the same scrape engine as the app,
with no window involved. Useful for running a sync from a terminal, in a cron
job, or while debugging.

```sh
bun run cli          # build src-tauri/target/release/oculus
bun run cli:install  # build + symlink into /usr/local/bin
```

```sh
oculus                          # session, sidecar and library status
oculus auth login               # opens the app's Canvas sign-in, waits for the session
oculus auth logout              # forget the session and the SSO profile

oculus list -s                  # subjects (● = current term)
oculus list -s --refresh        # re-fetch the course list from Canvas first
oculus list -l MULT20015        # lectures for a subject

oculus run -s                   # scrape every selected current subject
oculus run -s MULT20015 COMP30026
oculus run -s --all             # include past terms
oculus run -s --no-embed        # parse PDFs but skip the retrieval index
oculus run -s --no-parse        # skip the sidecar entirely

oculus index                    # re-parse + re-embed PDFs already on record
oculus index MULT20015
oculus index --memory-cap 8192   # live whole-sidecar-tree cap, MB; minimum 5120

oculus run -l MULT20015                 # sync the lecture list
oculus run -l MULT20015 --transcripts   # + download VTTs
oculus run -l MULT20015 --videos        # + download and trim the videos
```

`run -s` also refreshes each subject's Canvas calendar — class times and
assignment due dates — into the database, which is what the app's Calendar
page reads.

`run -s` scrapes, then parses every PDF it wrote and folds it into the
retrieval index — one file at a time. Local model work shares one queue;
opt-in MinerU cloud quality is batched separately. Both halves are
idempotent, so re-running costs almost nothing.

The sidecar returns as soon as its *fast* pass has produced markdown and
continues with the slower, better parse in the background. That improved text
is not in the database yet when the command exits; `oculus index` picks it up
without re-downloading anything. If the sidecar is not running, the scrape
still completes and says so.

`--memory-cap` requires the sidecar to be running and changes its current
budget without restarting it. It is not persisted; Settings → Library owns
the saved budget and backend choice. The default is 8 GB for the entire
sidecar tree, not per worker. The 5 GB floor is allowed but does not guarantee
that local quality parsing will fit. Cloud processing is off by default.

Subject codes match on the prefix, so `MULT20015` finds
`MULT20015_2026_SM2`. The CLI reads the same session cookie and writes the
same `oculus.db` the app uses, so a CLI sync shows up in the app and vice
versa — but the database must already exist, which means opening the app once
on a fresh machine.

Signing in genuinely needs a browser: Canvas authenticates through the
university's SAML IdP. `oculus auth login` launches the app for that step and
polls for the cookie it saves.

## UI

Components come from [shadcn/ui](https://ui.shadcn.com) and live in
`src/components/ui`. They are source, not a dependency: edit them in place.
Configuration is in `components.json`.

- **Primitives**: `radix-ui` (unified package)
- **Icons**: `@phosphor-icons/react` — shadcn generates lucide imports, so swap
  them to Phosphor after adding a new component
- **Theme tokens**: `src/index.css`. The shadcn token names (`background`,
  `card`, `popover`, `secondary`, `accent`, `input`, `ring`, …) are mapped onto
  the project's own Linear-style grey + indigo palette, so stock shadcn
  components inherit the app's look with no per-component overrides.
- **Dark mode**: a `.dark` class on `<html>`, driven by `src/lib/theme.ts`.
  `index.css` declares `@custom-variant dark (&:is(.dark *))` so `dark:`
  utilities follow that class rather than the OS setting.

## Recommended IDE setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

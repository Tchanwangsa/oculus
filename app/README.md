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
bun run cli:install  # build + symlink into ~/.local/bin
bun run docs:cli     # regenerate ../docs/cli-reference.md from the binary's help
```

`docs:cli` also runs as part of `bun run tauri build`, after the CLI has been
staged — so a release bundle can never ship a CLI that the checked-in
reference does not describe.

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

oculus run -l MULT20015                 # sync the lecture list
oculus run -l MULT20015 --transcripts   # + download VTTs
oculus run -l MULT20015 --videos        # + download and trim the videos
```

### Querying the library

Read-only commands, for a terminal or for a coding agent. Every one of them
takes the global `--json`.

```sh
oculus search "kkt conditions" -s MULT20015 -n 8   # rank pages by meaning
oculus search "regular expressions" --full         # whole pages, not snippets

oculus grep "sprint retrospective"                 # regex over all scraped text
oculus grep "P(A | B)" -F -s COMP30026             # -F: literal, not a regex
oculus grep todo -l                                # matching paths only

oculus read 13.pdf --pages 30-35                   # parsed page markdown
oculus read courses/COMP30026_2026_SM2/home.md     # or a file on disk

oculus files COMP30026 --type pdf                  # what is there to read
oculus files --category ed -m assignment

oculus calendar -d 14                              # next fortnight
oculus calendar COMP30026 --due                    # due dates only

oculus docs                                        # regenerate the agent CLI reference
```

`search` ranks page *images* through the sidecar's embedding model, so it
needs the sidecar running — in practice, the app open. When it is not, the
command fails with exit 1 and points at `grep`, which needs nothing and
searches the same text literally.

`grep` is not interchangeable with ripgrep over the library folder: markdown
is on disk, but PDF page text lives only in `oculus.db`, so ripgrep misses
every slide deck. `grep` reads both.

`docs` fills `agents/` in the data directory for coding agents working in a
course folder: a CLI reference rendered from this binary's own help, and one
`AGENTS.md` symlinked into every course. `bun run cli:install` runs it, so the
reference always matches the installed binary. It never overwrites
`OCULUS.md`, `TASTE.md`, or a course's own `agents/INSTRUCTIONS.md`.

Every sync does the linking half itself, so a newly enrolled subject arrives
ready to work in. Run `docs` when you want the CLI reference refreshed.

`read` takes a full library path, a bare filename, or a distinctive fragment,
and lists the candidates rather than guessing when a fragment is ambiguous.
Its `--pages` numbers are the ones `search` reports.

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

Settings → Library owns the parser choice: MinerU's cloud service, or a
MinerU server you install and run yourself, reached over loopback. Neither is
this app's child process, so there is no memory budget to set from here — the
`--memory-cap` flag went with the Python sidecar it bounded.

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

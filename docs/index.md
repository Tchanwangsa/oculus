# Oculus docs

Oculus is a Tauri 2 desktop app that scrapes a student's coursework from any
university's Canvas — pages and files, and Ed Discussion threads, Ed Lessons and
Echo360 lecture recordings where the university uses them — into a local
library, parses every PDF to markdown, embeds each page as an
image, and retrieves the right pages for a question. Chat is a coding agent the
student already has (Claude Code, Codex, opencode or Antigravity), run inside
the library with the `oculus` CLI as its tool surface. These pages are the map;
the root `CLAUDE.md` holds only the rules.

## Reading order

| Page | What it covers |
| --- | --- |
| [architecture.md](./architecture.md) | The two processes and the credential broker, how they talk, the data directory, `oculus.db`, the code layout |
| [sync.md](./sync.md) | The scrape engine: Canvas modules, Ed threads and lessons, Echo360 lectures, HTML→md |
| [auth.md](./auth.md) | Canvas session in keyd's vault, sign-in on demand, Okta sign-in and its attempt guard, Ed `x-token`, Echo360 LTI, WebKit's WebCrypto master key in keyd and the data-store delegate that hands it to WebKit |
| [parsing.md](./parsing.md) | PDFs to markdown: the parser seam, the two MinerU engines, failures |
| [retrieval.md](./retrieval.md) | Page-image embeddings, the `pages` table, query flow |
| [harness.md](./harness.md) | Chat as a CLI agent: the four bridges, containment, the timeline |
| [calendar.md](./calendar.md) | Class times, deadlines and recordings on one grid |
| [projects.md](./projects.md) | Projects and tasks: the Overview, board, table, timeline and the universal Tasks view |
| [chapters.md](./chapters.md) | Lecture chapters and where a lecture ends: the detector and the two model jobs |
| [onboarding.md](./onboarding.md) | First-run setup: the launch gate, the `onboarding` settings row, the steps, *Run setup again* |
| [frontend.md](./frontend.md) | The frontend's data side: backend events, settings, parse state, library files |
| [shell.md](./shell.md) | Per-pane routers, tabs, the side panel, window shortcuts, ⌘F, search |
| [dock.md](./dock.md) | The messaging dock: WhatsApp, Instagram, Messenger and Discord as parked native pages, their data stores, the capability invariant, the Instagram one-hop guard |
| [ui.md](./ui.md) | The UI system: design rules, and the WebKit and CSS traps |
| [music.md](./music.md) | Spotify played natively through librespot: the session task, credentials in keyd, the `music-state` snapshot, Settings → Music, the rail tile and `/music` player, commands that control the device playing (here or another one), the output on this Mac, and why music and lecture audio never play together |
| [music-library.md](./music-library.md) | Playlists (the rootlist), their tracks and made covers (playlist contents, batched track metadata), search (pathfinder `searchDesktop`) and the 429 gate over the session's spclient, the queue and up next, and the Connect device list and transfer |
| [viewers.md](./viewers.md) | Markdown, PDFs, the media player (lectures, Up Next and library videos), transcription and the in-app browser |
| [editor.md](./editor.md) | The note editor: sessions, find, versions, code, tables, pictures, mentions, `NoteField` |
| [editor-maths.md](./editor-maths.md) | Maths in the note editor: the visual field, the toolbox, shorthands |
| [editor-core.md](./editor-core.md) | The Rust editor core, its oracles, and shadow mode checking it against every note editor in dev |
| [studio.md](./studio.md) | Studio's renderer: video folders, the TSX component contract, narration (script.md, the manifest, `narrate` and its TTS server, scene lengths, word cues), styles, motion, transitions and magic move, motion blur, contact sheets (`strip`), dithered gradients, the sandbox, lint and golden frames |
| [cli.md](./cli.md) | The `oculus` binary: what each command writes, and the agent docs it generates |
| [cli-reference.md](./cli-reference.md) | Every command and flag — generated from the binary, not hand-kept |
| [windows.md](./windows.md) | Windows platform boundaries, broker security, data paths and release acceptance |
| [development.md](./development.md) | Building, running and checking each piece |

## Repo layout

| Path | What it is |
| --- | --- |
| `app/src/` | React 19 frontend (Vite, Tailwind v4, shadcn/ui) |
| `app/src-tauri/src/` | Rust: Tauri commands, scrape engine, parsing, embedding, retrieval, the harness |
| `app/src-tauri/src/bin/oculus/` | The headless CLI over the same engine |
| `app/keyd/` | `oculus-keyd`, the credential broker, and `core/`, its OS-free logic and OS adapters — a separate cargo root |
| `app/editor-core/` | A standalone Rust port of the note editor's text model and markdown parser, checked against CodeMirror and Lezer; in dev builds, as WebAssembly, against every note editor |
| `app/studio-core/` | Studio's scene renderer: TSX components in a QuickJS sandbox, laid out with taffy and parley and drawn with vello_cpu — a separate cargo root the CLI links |
| `app/src-tauri/templates/` | The agent-facing docs and skills written into the library |
| `app/scripts/` | Dev preflight, native-binary fetchers, CLI staging and reference generation |
| `docs/` | These pages |
| `.agents/skills/` | Shared skills: `read-docs`, `write-docs`, `check-doc-drift` |
| `.claude/skills/` | Symlink to `.agents/skills/` for Claude Code |

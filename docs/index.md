# Oculus docs

Oculus is a Tauri 2 desktop app that scrapes a student's UniMelb coursework —
Canvas pages and files, Ed Discussion threads, Echo360 lecture recordings —
into a local library, parses every PDF to markdown, embeds each page as an
image, and retrieves the right pages for a question. Chat is a coding agent the
student already has (Claude Code, Codex, opencode or Antigravity), run inside
the library with the `oculus` CLI as its tool surface. These pages are the map;
the root `CLAUDE.md` holds only the rules.

## Reading order

| Page | What it covers |
| --- | --- |
| [architecture.md](./architecture.md) | The two processes and the credential broker, how they talk, the data directory, `oculus.db` |
| [sync.md](./sync.md) | The scrape engine: Canvas modules, Ed threads, Echo360 lectures, HTML→md |
| [auth.md](./auth.md) | Canvas session cookie, keep-alive, Okta sign-in and its attempt guard, Ed `x-token`, Echo360 LTI |
| [parsing.md](./parsing.md) | PDFs to markdown: the parser seam, the two MinerU engines, failures |
| [retrieval.md](./retrieval.md) | Page-image embeddings, the `pages` table, query flow |
| [harness.md](./harness.md) | Chat as a CLI agent: the four bridges, containment, the timeline |
| [calendar.md](./calendar.md) | Class times, deadlines and recordings on one grid |
| [projects.md](./projects.md) | Projects and tasks: the Overview, board, table, timeline and the universal Tasks view |
| [chapters.md](./chapters.md) | Lecture chapters and where a lecture ends: the detector and the two model jobs |
| [frontend.md](./frontend.md) | The frontend's data side: backend events, settings, parse state, library files |
| [shell.md](./shell.md) | Per-pane routers, tabs, the side panel, window shortcuts, ⌘F, search |
| [ui.md](./ui.md) | The UI system: design rules, and the WebKit and CSS traps |
| [viewers.md](./viewers.md) | Markdown, PDFs, the media player (lectures, Up Next and library videos), transcription and the in-app browser |
| [editor.md](./editor.md) | The note editor: sessions, find, versions, code, tables, pictures, mentions, `NoteField` |
| [editor-maths.md](./editor-maths.md) | Maths in the note editor: the visual field, the toolbox, shorthands |
| [cli.md](./cli.md) | The `oculus` binary: what each command writes, and the agent docs it generates |
| [cli-reference.md](./cli-reference.md) | Every command and flag — generated from the binary, not hand-kept |
| [development.md](./development.md) | Building, running and checking each piece |

## Repo layout

| Path | What it is |
| --- | --- |
| `app/src/` | React 19 frontend (Vite, Tailwind v4, shadcn/ui) |
| `app/src-tauri/src/` | Rust: Tauri commands, scrape engine, parsing, embedding, retrieval, the harness |
| `app/src-tauri/src/bin/oculus/` | The headless CLI over the same engine |
| `app/keyd/` | `oculus-keyd`, the credential broker, and `core/`, its OS-free logic and OS adapters — a separate cargo root |
| `app/src-tauri/templates/` | The agent-facing docs and skills written into the library |
| `app/scripts/` | Dev preflight, native-binary fetchers, CLI staging and reference generation |
| `docs/` | These pages |
| `.agents/skills/` | Shared skills: `read-docs`, `write-docs`, `check-doc-drift` |
| `.claude/skills/` | Symlink to `.agents/skills/` for Claude Code |

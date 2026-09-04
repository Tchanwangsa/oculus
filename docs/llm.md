# LLM provider layer

Everything that talks to a language model: provider config, the model
library, API keys, the streaming client, and the spending ledger.

## Where

| Piece | Location |
| --- | --- |
| Client, keychain, budget, memory preflight | `app/src-tauri/src/llm.rs` |
| The agent tool loop | `app/src-tauri/src/agent.rs` |
| `llm_usage` table, `chats`/`chat_messages` | migrations 16–17 in `app/src-tauri/src/lib.rs` |
| Settings UI | `app/src/pages/settings/AiPage.tsx` |
| Provider row, model browser, pickers | `app/src/components/llm/` |
| Chat UI + store | `app/src/pages/ChatPage.tsx`, `app/src/stores/chatStore.ts` |
| Config read/write, chat reads | `app/src/lib/db.ts` (`getLlmSettings`, `getChats`) |

## How it connects

- **One client, every provider.** Ollama, OpenRouter, OpenCode Go and any
  custom base URL all speak the OpenAI-compatible surface
  (`/v1/chat/completions` with SSE, `/v1/models`), so there is a single code
  path and "add a provider" is a base URL plus a key. Kind defaults live in
  `ProviderConfig::base_url`; only `custom` requires the user to supply one.
- **Add provider offers three presets and Custom.** OpenCode Go, OpenRouter
  and Ollama are the endpoints worth a one-click default; everything else is
  Custom, which is the same code path with the URL typed in. `PROVIDER_KINDS`
  in `app/src/lib/db.ts` is the registry of kinds the client *understands*,
  `ADDABLE_PROVIDER_KINDS` the subset it *offers* — LM Studio sits in the gap
  (`addable: false`), so a config that already names it keeps its label and
  its `http://localhost:1234` default while new ones reach it through
  Custom.
  That base URL is stored as the endpoint *root* — a pasted `/v1` suffix is
  trimmed on both sides (`ProviderConfig::base_url`,
  `app/src/components/llm/ProviderCard.tsx`), because the request builder adds
  the `/v1` itself and providers advertise their URL both ways.
- **OpenCode Go serves three request formats; we speak one.** Its
  subscription gateway (`https://opencode.ai/zen/go`) routes some models to
  `/v1/chat/completions`, others to Anthropic's `/v1/messages`, and the
  frontier ones to OpenAI's `/v1/responses` — but `/v1/models` lists all of
  them. Only the chat-completions models work here (the GLM, Kimi, DeepSeek,
  MiMo and Hy3 lines at the time of writing); picking a MiniMax, Qwen-max or
  Grok/GPT entry from the browser fails at send time. Adding the other two
  formats would mean a second and third request builder, which is the one
  thing this layer has stayed free of.
- **Any number of providers, at once.** Config holds a list of
  `ProviderConfig`s, each with a generated `id` that never changes — it is
  the keychain account for that provider's key, so regenerating it would
  orphan the key. Two accounts of the same kind are just two entries
  (`openrouter`, `openrouter-2`).
- **Models are chosen from a library, not from a provider.** A `ModelRef` is
  a (provider id, model id) pair, and every model setting — chat, summary,
  each fallback, the chat composer's switcher — points into
  `LlmConfig::library`. Browsing a provider's `/v1/models` is a separate act
  with its own dialog: OpenRouter lists hundreds, and none of them should
  have to be scrolled past to pick the two you use. The catalogue is fetched
  on demand and never stored; only the library is.
- **Removing a provider removes its models.** Settings prunes the library,
  the defaults and the fallback chain in the same write — a `ModelRef`
  pointing at a provider that no longer exists is a call that fails at send
  time, and `resolve` treats one as a skipped candidate for the same reason.
- **Config is in the `settings` table under the `llm` key**, written by the
  frontend and read back in Rust via sqlx — the same split as the scrape
  tables. The TS `LlmSettings` interface and the Rust `LlmConfig` struct are
  the same JSON (serde camelCase); change one and the other moves with it.
  Both sides upgrade the pre-multi-provider shape (one provider, three bare
  model names) on read, in memory, and the first Settings edit writes the new
  shape back — so a stale row never has to be migrated by a schema step.
- **API keys never enter the database or the frontend.** They live in the
  macOS keychain (service `com.tchan.oculus.llm`, account = provider id) and
  the commands only ever return booleans about them. A key is read inside
  the request builder and nowhere else. Local providers need none.
- **The budget check is the reason usage is recorded in Rust.** Every call
  path sums the current calendar month from `llm_usage` *before* it issues a
  request, and writes a row after — so a limit stops the next call rather
  than reporting damage after the fact. Later, the agent loop checks before
  every model turn, not just once per conversation.
- **Cost comes from the provider or not at all.** Requests ask for usage
  in-stream (`stream_options.include_usage`, plus `usage.include` on
  OpenRouter, which is what actually returns a dollar figure). Local
  providers record NULL cost and token counts only — there is no pricing
  table and no tokenizer in Rust, so context is guarded by character caps.
- **Streaming stays on ureq**, the crate's only HTTP client: SSE is lines on
  `resp.into_reader()`. The request sets a *read* timeout but no overall
  timeout — a local model can legitimately sit silent for a minute loading
  before the first token, and an overall timeout would kill long
  generations. The cost is that a hung provider blocks its thread until the
  read timeout, so cancellation can only take effect between reads.
- **Tool-call deltas must be accumulated, not read.** Providers split a
  single call's `arguments` JSON across many chunks keyed by `index`;
  `chat_completion_stream` merges them and returns one assistant message
  ready to append to history. Nothing downstream should parse raw chunks.
- `llm_test_prompt` exists so the whole path — config, keychain, SSE,
  usage, budget — is exercisable from Settings → AI before any feature
  depends on it. It takes a `ModelRef`, because with several providers
  configured "does this one work" is a per-model question. It streams through
  the `llm-test-delta` event, the same Rust-emits/frontend-folds pattern as
  scrape progress (see [architecture.md](./architecture.md)).
- **Every call site records its own usage kind.** The chat agent writes
  `chat`, `llm_test_prompt` writes `test`, so a model you were only trying out
  never muddies what conversations actually cost. The kind column is the seam
  the removed automations feature used for its background spend (`summary`,
  `automation`); old rows still carry those, and a new background caller should
  add its own kind rather than borrow `chat`.

## Local models must fit before they load

A local model that does not fit is not slow, it is an OOM that takes the
machine with it — observed with a 17 GB model loading beside the sidecar's
Qwen3-VL embedder on a 36 GB machine. So `resolve` runs before every local
call:

- Model size comes from Ollama's **native** `/api/tags` — the
  OpenAI-compatible `/v1/models` carries no size, and LM Studio exposes none,
  so an unmeasurable model skips the check rather than blocking on a guess.
- Available memory is **physical memory less wired pages** (`sysctl
  hw.memsize` and `vm_stat`), less a 2 GB reserve. Not free pages: macOS keeps
  almost nothing free, compressing and evicting on demand — measured, a 36 GB
  machine serving a 17.4 GB model reported 2.9 GB free, a figure that refuses
  every model in the library. Wired pages are the ones that cannot be paged
  out, and on Apple silicon that is where GPU-resident weights (and the
  sidecar's MPS embedder) sit, so they are the ones worth counting. Read the
  page size out of `vm_stat`'s header; it is 16 KB on Apple silicon, and
  assuming 4 KB miscounts fourfold.
- Whatever Ollama already holds loaded (`/api/ps`) counts as available — it
  evicts to make room — and a model **already resident needs nothing at all**,
  so it is never refused whatever the arithmetic says.
- Weights are budgeted at 1.15× for the KV cache and runtime (measured: a
  16.5 GB Q4_K_M 27B is 17.4 GB resident at a 32k context).
- Too big → the next model in the fallback chain, in the user's order, until
  one fits; a refusal names every candidate and its numbers. Refusing happens
  *before* the user's message is written, so a doomed turn leaves no history
  behind.

The chain is the preferred model followed by `LlmConfig::fallbacks`, capped
at `MAX_FALLBACKS` (5) on both sides. It is short deliberately: it is a list
to read at a glance and drag into order, not a routing table. Adding a sixth
in Settings drops the last rather than opening a dialog about which to evict.

## The agent loop

- Tools reach the same data the UI does — `search_library` calls
  `retrieval::search` directly, `read_file` reads the parsed markdown through
  `files::read_parsed_markdown`, which is why both are free functions rather
  than command-only code. Results are capped (`MAX_TOOL_CHARS`) because a
  tool result is prompt text, and one reading pack would otherwise blow the
  window.
- The loop stops after `MAX_TOOL_ROUNDS`, and that final turn is issued with
  the tools withheld — a model that keeps searching is forced to answer
  rather than looping until the budget dies.
- **The model is fixed per send, not per chat.** `chat_send` takes an
  optional `ModelRef` from the composer's switcher and resolves it once,
  before the user's row is written; the loop then runs every round on that
  one provider. Switching mid-thread therefore changes what follows and
  leaves the history alone — which is also why the model is stored on each
  message row rather than on the chat.
- **Rust writes the message rows.** The assistant's tool-call turn is
  persisted *before* its tools run, so a crash mid-round cannot leave a
  history whose tool results reference a call that was never recorded.
  `getChatMessages` in `app/src/lib/db.ts` hides those plumbing turns from
  the reader; the loop reads all of them.
- Citations travel two ways: the model is asked to link inline with an
  `oculus-file://` URL, which `ChatPage` intercepts and opens locally, and
  every file a tool surfaced is recorded on the final message as a fallback
  chip row. Chips dedupe on **filename**, not path — Canvas serves the same
  document from both a module folder and the files list.

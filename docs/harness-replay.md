# Provider replay uses the production store and rows

The offline testkit exercises recorded CLI traffic without provider processes,
network requests, Tauri startup or the student's database. It is the baseline
and release gate for the agent turn model rebuild.

## Where

| Piece | Location |
| --- | --- |
| Fixtures, individual manifests and explicit known failures | `app/src-tauri/fixtures/harness/turn-model/` |
| Replay runner (`replay.rs`), its step-by-step runtime (`rig.rs`, `io.rs`), scratch guard and assertions | `app/src-tauri/src/harness/testkit/` |
| Process transport substitution | `app/src-tauri/src/harness/transport.rs` |
| Provider constructors and inbound runners | `app/src-tauri/src/harness/providers/claude/replay.rs`, `app/src-tauri/src/harness/providers/codex/replay.rs`, `app/src-tauri/src/harness/providers/opencode/replay.rs`, `app/src-tauri/src/harness/providers/antigravity/replay.rs` |
| Shared ordered consumer and injected external effects | `app/src-tauri/src/harness/app/consumer.rs` |
| Headless frontend store and row projection | `app/tests/harness/project-replay.ts` |
| Replay output the timeline's own tests fold | `app/tests/fixtures/turn-model/`, `app/tests/components/harness/runRows.test.ts` |

## Replay substitutes transport and external effects

`Scenario::recorded(name)` loads and validates a manifest and its raw JSON
frames. `mutate(Mutation)` explicitly changes a recording for robustness
checks; mutations are never described as captured provider behavior.
`run(scenario)` drives a `Rig` (below) through the whole recording. The rig
creates a migrated scratch SQLite file, registers production
provider handles as the manager's next spawn and starts the production consumer on its own
thread, fed by a bus as in the app. It persists the send's run through the
production `begin_run` on that bus (receipt `replay-send`) and then calls
`Harness::send`; a refused intent skips the send, as the app does. The normal provider
encoders write into checked transports. Recorded inbound frames use the
production translators (Codex and opencode also use their production route
dispatchers). The emitted stream goes through the same ordered consumer as
the app, with deterministic thought-clock ticks.

The runner captures each frontend envelope and database snapshot after every
event, final rows, reopened rows, outbound frames, queue state and the
thread's runs, attempts, bindings, receipts and delivery gaps
(`snapshot::ledger`, runs with their answer, delivery and interval). Rows
carry their `run_id`, `attempt_id`, `entity_ref`, `source_ref`,
`native_phase` and `lifecycle`; live/reload comparison keeps `run_id`, which
the timeline folds by, and drops the rest. `project` feeds those envelopes
into the production `useHarnessStore` and `buildRows` using a short-lived Bun
process: live rows fold with the store's runs, reopened rows with the
ledger's, as `getHarnessRuns` would read them. Comparison ignores wall-clock display
creation times and the frontend-omitted question anchor, and parses JSON
metadata before comparing. SQL reopen comparison includes both fields. The load-race scenario substitutes
only the SQL plugin transport and native queue read, then lets production `load`
overwrite changes received while its snapshot is pending. Widget progress uses
the production widget store with in-memory storage and original row/fence keys.

Process spawning is prohibited by the replay manager. Process-pipe writes and
HTTP helpers consume an exact ordered outbound expectation list. A mismatch,
extra operation, missing expected write, queued dispatch or naming attempt
fails closed. The test database explicitly disables naming and other jobs.
No provider CLI is discovered or started. Claude replay clears the optional
transcript lookup cwd between frames, so its anchor-file fallback cannot read
user transcripts; the input frames themselves are unchanged.

Every writable path is guarded before opening, including SQLite's own
`PRAGMA database_list` paths. The guard resolves symlinks and nonexistent tails,
rejects the live Oculus data directory and main checkout, and requires scratch
containment. Tests exercise direct, escaped and symlinked forbidden paths
without opening their target files. Scratch pools, Bun processes and temporary
directories are closed or removed before the runner returns.

## A test can act mid-turn

`Rig` (`testkit/rig.rs`) is the runtime `run` uses, one step at a time:
`start` (the scratch database, the consumer thread, the recording's bridge),
`open` (the recording's request and the scenario's storage faults), `feed`
and `resume` (inbound frames), `eof`, `close` and `finish` (the `Replay`).

A **gate** pauses a transcript at a labelled frame. `Scenario::gate(label,
pick)` labels the first frame `pick` matches; `rig.until(label)` feeds the
recording up to that frame, leaves it unfed, and returns once the consumer
has applied everything sent so far (`Rig::drain`: the feed answers when it
finds the bus empty, with no sleep to tune). The consumer's clock is the
rig's (`io.rs`, `Clock`): when the consumer has asked to be woken and the
bus is empty, the feed moves the clock to that moment and delivers the
`Wake`, so work that waits on time (a draft write, a settle retry) runs
without a real timer. `testkit/pacing.rs` scripts that clock message by
message. `rig.read_only(true)` makes every write fail, as a full disk would
(`PRAGMA query_only` on the rig's one connection), and
`rig.await_wakes(n)` waits for the consumer's retries
(`testkit/recovery.rs`, which also sends during startup). The test then does what a
student or the app would do mid-turn, such as a second `rig.send` or a call
on `rig.harness`, and `resume`s. Races are tested this way, never by timing.

The manager never spawns in a replay. A recorded session is registered as the
thread's next spawn (`Harness::replay_spawn`) and opened by the send that
would have started a process, on the sink of the generation minted for it.
`rig.next_session()` registers another and hands back the one it replaces,
whose reader the test can go on feeding: a process the app no longer listens
to. A replayed process records a kill, so `handle.is_alive()` says whether
the manager ended it.

```rust
let scenario = Scenario::recorded("codex-ls")
    .gate("terminal", |f| f["method"] == "turn/completed");
let mut rig = Rig::start(scenario);
rig.open();
rig.until("terminal"); // the turn is open here
rig.resume();
rig.close();
let replay = rig.finish();
```

A **restart** is `rig.restart()`: the consumer is joined, the manager shut
down and the pool closed, as a killed app would leave them; then the same
database file is opened through the production pool (`db::store::pool`) and
a new manager, consumer and recorded session come up. Nothing has repaired
anything yet. `rig.startup()` runs the production startup path
(`app/startup.rs`, the gate `lifecycle::reconcile` and every thread command
go through), and `rig.send` waits at that gate first, as `harness_send`
does, so a test can send before the repair and see which ran first. A test
of what a restart does to runs goes through that gate, never
`store::reconcile` alone. `Replay::envelopes` keeps
what every runtime emitted, in order.

```rust
let mut rig = rig.restart();          // killed mid-answer, started again
let second = rig.send("after-restart").unwrap(); // waits for the repair
```

## Manifests preserve evidence boundaries

Each manifest declares provider, CLI/protocol version, model, capture date or
its uncertainty, source range, redaction, scenario, raw-frame count, SHA-256,
and expected outbound frames/responses. The existing thirteen bridge recordings
are referenced unchanged. Exact CLI versions not present in a recording remain
unknown; a bridge protocol version is labeled as such.

Six new excerpts retain every frame in a bounded Claude episode. Private text
uses a stable character substitution preserving whitespace, scalar counts,
chunk boundaries, equality, punctuation, IDs, parent links, signatures and frame order.
Input-JSON chunks retain key names, escapes and primitive syntax.
Non-ASCII private characters become `◇`, so UTF-8 byte widths can change.
These redacted bodies test structural delivery, not answer quality. Full
private threads are not fixtures.

| New excerpt | Private asset range | Frames | Signature-bearing frames |
| --- | --- | ---: | ---: |
| claude-144-duplicate-a | thread-144:439–619 | 181 | 6 |
| claude-144-duplicate-b | thread-144:622–769 | 148 | 6 |
| claude-158-short-final | thread-158:1680–1924 | 245 | 3 |
| claude-158-gap | thread-158:2603–2679 | 77 | 9 |
| claude-159-recovery | thread-159:2018–2202 | 185 | 9 |
| claude-160-gap | thread-160:2130–2241 | 112 | 9 |

The schema-upgrade replay pins SHA-256 for migrations 1–54, migrates legacy
assistant rows from schema 24, checks source IDs/bodies, that they stay
unowned after migration 54, and proposal source keys, then drives production
rewind/delete pruning. `testkit/identity.rs` replays the holes turn and session ids close (a second
Codex `turn/completed` under the next run, a replaced process speaking into
its successor's run, a send racing a turn Claude starts itself, a
reasoning-level change leaving two processes, a killed process's last words
after the next question started, a process dying before or during its
answer).
`testkit/runs.rs` checks run identity: one run,
attempt, binding and receipt per send on all four bridges, with every later
row owned by them; a repeated receipt starting nothing; a failed intent
blocking the provider write; bindings following the native session; rewind,
delete and restart handling of runs; a stray row on the bus written before
the next question; every bridge naming its messages with no gap on clean or
stopped turns; a stream cut off without a stop kept as partial with a gap;
and thread naming and status following the settled run. New migrations may append;
existing pinned SQL must remain byte-identical.

## Known failures are executable release gates

Run in the implementation worktree, one heavy Cargo command at a time:

```sh
cd app/src-tauri
RUSTC_WRAPPER= CARGO_BUILD_JOBS=2 nice -n 15 cargo test harness::
RUSTC_WRAPPER= CARGO_BUILD_JOBS=2 nice -n 15 cargo clippy --all-targets -- -D warnings
cargo fmt
```

The normal `known_failure_matrix_is_exact` test executes every row in
`matrix.json` and requires its observed failures to equal `expected_failures`.
Both new failures and unexpectedly passing gates fail the baseline. Remove a
known failure only when implementing its invariant. Every row currently
passes; the ignored `release_matrix_has_no_known_failures` test requires that:

```sh
RUSTC_WRAPPER= CARGO_BUILD_JOBS=2 nice -n 15 cargo test --lib -- --ignored --exact harness::testkit::matrix::release_matrix_has_no_known_failures
```

The invariant numbers below refer to the turn-model design: 1 durable command,
2 scoped identities, 3 root settlement, 4 one selected answer, 5 honest terminal
outcome, 6 unresolved gap, 7 artifact preservation, 8 replay/reload idempotency.
A passing cardinality check alone does not establish a persisted answerRef;
the first row gates that contract separately for all bridges.

| Matrix row | Baseline | Broken invariant |
| --- | --- | --- |
| durable-command-and-answer-reference | Pass | — |
| claude-144-duplicate-a | Pass | — |
| claude-144-duplicate-b | Pass | — |
| edited-second-answer | Pass | — |
| answer-before-trailing-tool | Pass | — |
| short-terminal-message | Pass | — |
| claude-158-gap | Pass | — |
| claude-160-gap | Pass | — |
| claude-159-recovery | Pass | — |
| public-narration-mutation | Pass | — |
| claude-ls | Pass | — |
| codex-ls | Pass | — |
| opencode-v1-ls | Pass | — |
| antigravity-ls | Pass | — |
| claude-ls-repeat-snapshot | Pass | — |
| codex-ls-repeat-snapshot | Pass | — |
| opencode-v1-text-repeat-snapshot | Pass | — |
| antigravity-ls-repeat-snapshot | Pass | — |
| identical-distinct-messages | Pass | — |
| codex-native-phases-and-plan-mutation | Pass | — |
| claude-interrupt | Pass | — |
| codex-interrupt | Pass | — |
| opencode-v1-interrupt | Pass | — |
| opencode-unknown-reasoning-mutation | Pass | — |
| agy-refusal-success-mutation | Pass | — |
| claude-background-agent | Pass | — |
| claude-background-agent-fast | Pass | — |
| missing-terminal | Pass | — |
| restart-during-stream | Pass | — |
| persist-before-emit-failure | Pass | — |
| repeated-cursor-envelope | Pass | — |
| artifact-source-before-tool | Pass | — |
| compaction-only-mutation | Pass | — |
| repeated-root-terminal | Pass | — |
| codex-websearch-recording-without-terminal | Pass | — |
| runtime-exit-during-stream | Pass | — |
| persist-before-send-failure | Pass | — |
| after-send-before-ack-failure | Pass | — |
| snapshot-N-with-updates-during-load | Pass | — |

`answer_outside_fold` requires the run's `answer_item_id` to be the one reply
outside its fold, every other reply inside it; `card_outside_fold` requires a
folded reply's `openui` fence to draw once, in an `artifacts` row, and never
as an item row.

The ignored `write_timeline_fixtures` rewrites
`app/tests/fixtures/turn-model/` (envelopes, reopened rows, runs) for a list
of matrix cases (`TIMELINE_CASES` in `matrix.rs`); `runRows.test.ts` replays
them through the store and checks the fold states and fence splitting. Rerun
it, by its full name, after a bridge or settlement change:

```sh
RUSTC_WRAPPER= CARGO_BUILD_JOBS=2 nice -n 15 cargo test harness::testkit::matrix::write_timeline_fixtures -- --ignored --exact
```

The registry also replays the plain opencode text/reasoning/error recordings.
Interrupt fixtures assert retained public bytes, one outcome and one settlement.
Background recordings assert no extra student message, one autonomous divider
and live/reload equality. The incomplete Codex websearch recording deliberately
keeps its missing-terminal failures visible.

Native steering, two-correction races, superseded attempts, authoritative gap
replacement, transport reconnect offsets and actual fixed-protocol narration
capture remain unverified. The current event contract cannot represent several
of those facts. Artifact tests cover source visibility, the fence drawn once
outside its fold with its original index, stable database keys, widget
progress and absent Ask grants; proposal registration and Undo are covered
only through those keys, not by mounting a card, and superseded-output
folding needs superseded attempts. No mutation proves protocol support.

## Gotchas

- Claude's optional transcript-file anchor lookup is disabled in replay; anchor-file integration needs an injected scratch transcript resolver.
- The app's send preparation opens its default database; replay enters at `begin_run` and the manager, so it does not cover the Tauri command's thread lookup or prompt assembly.
- `durable_receipt` needs the thread's one receipt to name its run, not just the table.
- `gap_survives_reload` reads `harness_delivery_gaps` for the run, not the notice row settlement writes from it.
- `codex-ls` has two distinct messages, so its repeat-snapshot row checks `two_messages`: the repeat must add no third.
- `phase_and_plan` needs both Codex phases stored and the plan as one `Plan` tool row, never an assistant message.
- Every recording ends with its bridge's production EOF path (`Bridge::eof`), as a closed transport would; a `restart` scenario skips it, restarts the rig and runs the startup gate instead.
- After a restart the checks read the reloaded rows (`Replay::restarted`), since a reload is the app's only view then.
- The load-race projection strips `sequence`: its fresh store replays a stream whose sequence numbers the first pass already consumed.
- A bridge's repeated terminal names a turn that is already over and is dropped whatever the thread holds. Only the app's own terminal, which names no turn, goes by the claim: with no run open it releases the thread's claim, and with neither it is dropped.
- A Codex manifest answers `turn/start` with the recorded turn's own id, as the server does. The run is bound to that id, so a placeholder would leave the recording's `turn/completed` naming a turn nobody sent.
- Run only the named offline release gate with `--ignored`, by its full path; the bridge suite also contains ignored live provider tests (a real opencode thread spends OpenRouter credit). Cargo ORs a filter before `--` with one after it, so `cargo test harness:: -- --ignored release_matrix` runs every ignored `harness::` test.

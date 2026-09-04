# The `oculus` CLI

A second binary in `app/src-tauri` that drives the same scrape engine as the
app, with no window involved. Useful for terminal syncs, cron jobs, and
debugging. `app/README.md` carries the full command reference; this page is
how it fits the architecture.

## Where

| Piece | Location |
| --- | --- |
| The binary | `app/src-tauri/src/bin/oculus.rs` |
| Shared path resolution | `app/src-tauri/src/paths.rs` |
| Headless DB writes | `app/src-tauri/src/store.rs` |
| Build scripts (`cli`, `cli:install`) | `app/package.json` |

## How it connects

- `--memory-cap <MB>` is global, so `oculus --memory-cap 8192 index` and
  `oculus index --memory-cap 8192` are equivalent. Minimum 5120. It calls
  the running sidecar's `/limits` before the command, requires that sidecar
  to be available, and is not saved to the app's preferences. The budget
  covers the whole sidecar tree; at 5 GB local quality may not fit.
- The CLI reads the same session cookie and writes the same `oculus.db` the
  app uses — a CLI sync shows up in the app and vice versa. But it **never
  creates the database** (schema stays with the app's migrations), so a
  fresh machine must open the app once first; until then the CLI scrapes to
  disk and says so.
- `oculus auth login` launches the app for the SAML browser step and polls
  for the cookie the app saves — that path needs the app because a push or
  biometric challenge needs a human.
- `oculus auth setup` stores the username, password and TOTP setup key that
  let `oculus auth auto` do the whole Okta sign-in headlessly, no app and no
  browser. `oculus auth forget` clears them. See [auth.md](./auth.md) for
  what the setup key is and why it cannot be derived from codes.
- `oculus auth tick` is one keep-alive cycle — roll the session forward, and
  rebuild it headlessly if Canvas has rejected it. It is what the macOS
  LaunchAgent runs on a schedule, so it prints nothing, writes to
  `session-keepalive.log` in the data dir, and always exits 0: launchd has no
  console, and a non-zero exit only reads as a crashed job. Safe to run by
  hand when you want to know whether the agent's path still works.
- `run -s` scrapes, then parses and embeds each written PDF **one file at a
  time**. The sidecar serializes local heavy work but batches cloud quality
  independently. Both halves are idempotent; re-running is cheap.
- The sidecar returns once its *fast* pass has markdown; the quality parse
  finishes in the background after the command exits. `oculus index` folds
  that improved text into the database without re-downloading anything.
- `run -s` also refreshes each subject's Canvas calendar (class times and due
  dates) into `calendar_events` after the scrape — the CLI has no sync options
  to gate it with, so it always runs. See [calendar.md](./calendar.md).
- Subject codes match on prefix (`MULT20015` finds `MULT20015_2026_SM2`).

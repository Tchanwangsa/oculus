# Auth & sessions

Three services, three credentials — but everything derives from one Canvas
session cookie.

## Where

| Piece | Location |
| --- | --- |
| Canvas sign-in, session persistence | `app/src-tauri/src/auth.rs` |
| Session probe (`Valid`/`Rejected`/`Unreachable`) | `app/src-tauri/src/canvas.rs` |
| Auth flag, keep-alive log | `app/src-tauri/src/paths.rs` |
| Headless Okta sign-in, TOTP, stored credentials | `app/src-tauri/src/okta.rs` |
| Cookie/flag file locations | `app/src-tauri/src/paths.rs` |
| LaunchAgent keep-alive (app closed) | `app/src-tauri/src/keepalive.rs` |
| The `auth tick` the agent runs | `app/src-tauri/src/bin/oculus.rs` |
| In-app keep-alive loop + startup probe | `app/src-tauri/src/lib.rs` |
| Ed token minting via LTI | `app/src-tauri/src/ed.rs` |
| Echo360 session via LTI | `app/src-tauri/src/echo360.rs` |
| Frontend auth state | `app/src/hooks/useAuth.ts`, `app/src/hooks/useKeepalive.ts` |
| Credential entry UI | `app/src/components/settings/AutoSignIn.tsx` |

## Canvas

- **API tokens are not an option.** UniMelb Canvas has disabled self-service
  access tokens (the "New Access Token" button renders `disabled`; only an
  admin can issue one). The app therefore authenticates every request with a
  snapshotted `canvas_session` cookie. Do not re-propose `Bearer` auth
  without re-verifying that admin setting — and note the button's *string*
  is present either way; only the `disabled` attribute tells you.
- **Sign-in genuinely needs a browser**: Canvas authenticates through the
  university's SAML IdP, so login happens in a real (visible) app WebView.
  The captured cookie is persisted to the data dir in plaintext, alongside
  an auth-flag file whose presence means "we believe we have a session".
- **The session has no cookie-side expiry.** The server tracks lifetime and
  extends it on use, so periodic requests are what keep it alive. There is
  no remember-me cookie; once dead, the session must be rebuilt from
  scratch — either by [automated sign-in](#automated-sign-in) or by hand.

### Keep-alive, two layers

- **App open**: a thread in `app/src-tauri/src/lib.rs` re-probes the saved
  session every 6 hours and emits `canvas-auth-expired` if it is rejected.
- **App closed**: `app/src-tauri/src/keepalive.rs` installs a macOS
  LaunchAgent that runs `oculus auth tick` on a schedule via launchd. The
  tick probes, re-signs-in on rejection, and reports into
  `session-keepalive.log` in the data dir — launchd has no console, and a
  non-zero exit would only read as a crashed job, so every outcome is a log
  line and exit 0.

  The agent was once a standalone `/bin/sh` script that re-implemented the
  cookie merge in awk, and it could only *ping*: on 401 it had nowhere to go,
  so it logged the failure and gave up. Weeks of `ping failed: HTTP 401` is
  what a keep-alive that cannot re-authenticate looks like. Running the CLI
  means the agent shares one probe, one cookie merge, and one sign-in with the
  app.

  **The CLI ships as a Tauri sidecar**, declared in `externalBin` alongside
  ffmpeg and staged by `app/scripts/stage-cli.mjs` from `beforeBuildCommand`.
  The bundler does copy *some* sibling cargo binaries into `Contents/MacOS/`
  — `retrieval_smoke` lands there unasked — but not `oculus`; the selection is
  undocumented and appears to key off the name, which is no basis for the one
  binary the keep-alive depends on. Staging it explicitly costs a build step
  and is deterministic.

  That build step has a chicken-and-egg: `tauri-build` validates every
  `externalBin` path, and it runs for *every* bin in the crate, `oculus`
  included — so building the CLI requires the staged CLI. `stage-cli.mjs`
  writes an empty placeholder for that one build and replaces it with the real
  binary, removing it again if the build fails, so a zero-byte sidecar can
  never reach a bundle.

  `cli_path()` then resolves the CLI as a sibling of `current_exe`
  (`Contents/MacOS/oculus`), falling back to `target/release/oculus` under
  `tauri dev`. The plist stores that path absolutely, so
  `keepalive::repair_path` re-points an installed agent on startup when the
  bundle has moved — an agent calling a binary that is gone still "runs" every
  six hours and is indistinguishable from a working one until the session
  dies.

  **A rejected probe is not the same as an unreachable one.** The tick
  re-authenticates only on `Rejected` — on `Unreachable` it logs and waits,
  rather than spending an Okta sign-in attempt to discover the wifi is down.

  Neither layer can beat an absolute session cap or a forced IdP re-auth; when
  those fire, the user signs in by hand.

  **The agent installs itself only once a headless sign-in has actually
  succeeded** (`keepalive::ensure_installed`, called from the success path of
  `run_sign_in`). The gate is deliberately not "credentials are stored":
  recovery needs the TOTP factor, and an org that answers only with Okta
  Verify push or WebAuthn cannot be driven from launchd — there the agent
  would wake every six hours, fail, and log noise forever. Turning the
  Settings switch off writes a `keepalive-disabled` marker, so the next
  automated sign-in does not put it straight back.

### Automated sign-in

`app/src-tauri/src/okta.rs` rebuilds a dead session without a browser. The IdP
is Okta at `sso.unimelb.edu.au`, running **Identity Engine** — its sign-in
widget is a thin client over a JSON state machine at `/idp/idx/*`, so the
whole exchange can run in Rust: introspect the login page's state token,
answer each *remediation* Okta offers, then replay the SAML app URL to get an
assertion to POST to Canvas.

The loop is written against remediation **names**, not a fixed script, because
the order Okta asks for factors in is a policy setting that can change without
notice. It answers two factors:

- **password** — from the macOS keychain.
- **TOTP** (Google Authenticator) — generated locally from a stored seed.

**Answer the challenge before reading the chooser.** OIE offers
`select-authenticator-authenticate` alongside *every* `challenge-authenticator`
as the "verify with something else" escape hatch, so taking the chooser as the
next step re-picks the same authenticator forever without ever answering it.
The loop therefore answers whatever `currentAuthenticator` says is being
challenged, and falls through to the chooser only when that is a factor it
cannot answer.

It cannot answer Okta Verify push or WebAuthn; those need a human, which is
the point of them. When Okta offers only those, `LoginError::UnsupportedFactor`
carries the labels it *did* offer, so a failure names its own cause instead of
needing a packet capture.

The state token bootstrapped from the login page is a ~5 KB JWE, and the page
names `stateToken` several times — in inline script logic as well as in the
widget config. Taking the *first* mention yields a fragment that introspect
rejects as "The session has expired", which reads like an expiry problem and
is not one; `state_token_candidates` therefore takes every quoted value of
plausible shape. Note also that `/idp/idx/introspect` is the one call whose
field is named `stateToken` — every later call echoes back `stateHandle`.

**A TOTP secret cannot be recovered by observing codes.** A code is
`HMAC-SHA1(seed, unix_time / 30)` truncated to six digits — a one-way function
of a 160-bit seed. The seed is shown once, at enrolment, as the QR code and
the "setup key" beside it; getting it means re-enrolling the factor. TOTP is
implemented in-tree (SHA-1, HMAC, RFC 6238) rather than pulled from a crate,
and pinned by the RFC 4226/6238 test vectors.

**Canvas issues an anonymous `canvas_session` to the first visitor**, before
any authentication — the SAML start collects one on its way to Okta. So
"do we hold a `canvas_session`?" is not a success condition; `complete_saml`
only accepts one after it has actually posted an assertion, and `sign_in`
then proves the cookie against `/api/v1/users/self` before persisting it. A
session is never written to disk unverified, so a failed automated attempt
cannot clobber a working one.

Both probe sites in `app/src-tauri/src/lib.rs` call
`okta::try_auto_recover` before declaring a session expired, so a lapsed
session is normally rebuilt silently. `useAuth().connect()` tries the same
path before opening the login window. Credentials are entered in Settings →
Canvas (`app/src/components/settings/AutoSignIn.tsx`) or via
`oculus auth setup`.

**Every path that establishes a session must write the auth flag.** The
startup probe reads it before it reads anything else — no flag means "fresh
session" and the cookie beside it is never even looked at. `paths::
mark_authenticated` is the one writer; `oculus auth auto` used to skip it,
which left a valid cookie on disk and the app still opening disconnected.

A rejected password clears the stored one rather than letting the keep-alive
replay a wrong password every six hours until Okta locks the account.

**Security shape.** Password and TOTP seed live in the same keychain on one
machine, so against anything already running as this user the second factor is
no longer a second factor — the same posture as a password manager that stores
TOTP beside the password. It does not weaken the account against anyone who is
not already on this Mac.

### Startup probe

On launch, if the auth flag exists, the app is *optimistic* (state =
connected) while a background thread replays the cookie server-side:
`Valid` confirms, `Rejected` clears the flag (the SSO profile is kept so
re-login is one tap) and emits `canvas-auth-expired`, `Unreachable` stays
optimistic — an offline start is not an expired session.

## Ed Discussion

Ed has no third-party OAuth; every API call carries the web app's `x-token`
JWT. `app/src-tauri/src/ed.rs` mints it automatically by walking the Canvas →
Ed LTI 1.3 launch (course tabs → tool page → `oidc_login` → Canvas
`/api/lti/authorize` → `launch` → one-shot `?_logintoken=` →
`POST /api/login_token`). Tokens live ~2 weeks, are renewed via
`POST /api/renew_token` on each sync, and a dead one is simply re-minted from
Canvas. `oculus auth ed <TOKEN>` remains as a manual override. The redirect
walk must keep cookies **per-host** — the Canvas cookie must never be sent to
edstem.org.

## Echo360

No stored credential at all: each session is minted on demand from the Canvas
cookie via the course's LTI external-tool form (see
[sync.md](./sync.md)). The app caches the resulting session per course in
`app/src-tauri/src/lectures.rs`.

# Auth & sessions

Three services, three credentials — all derived from one Canvas session
cookie.

## Where

| Piece | Location |
| --- | --- |
| Canvas sign-in, session persistence | `app/src-tauri/src/auth.rs` |
| Session probe (`Valid`/`Rejected`/`Unreachable`) | `app/src-tauri/src/canvas.rs` |
| Cookie (Canvas and Okta), auth-flag and keep-alive log paths; private writes, sign-out | `app/src-tauri/src/paths.rs` |
| Headless Okta sign-in, TOTP, stored credentials, the attempt guard | `app/src-tauri/src/okta.rs` |
| LaunchAgent keep-alive (app closed) | `app/src-tauri/src/keepalive.rs` |
| The `auth tick` the agent runs | `app/src-tauri/src/bin/oculus/auth.rs` |
| Staging the CLI into the bundle | `app/scripts/stage-cli.mjs` |
| In-app keep-alive loop + startup probe | `app/src-tauri/src/lib.rs` |
| Ed token minting via LTI | `app/src-tauri/src/ed.rs` |
| Echo360 session via LTI, per-course cache | `app/src-tauri/src/echo360.rs`, `app/src-tauri/src/lectures.rs` |
| Frontend auth state | `app/src/hooks/useAuth.ts`, `app/src/hooks/useKeepalive.ts` |
| Handing the session to the in-app browser | `app/src-tauri/src/browser.rs` |
| Keychain entry lifecycle (also used by MinerU, Voyage and Groq) | `app/src-tauri/src/credentials.rs` |
| Credential entry UI | `app/src/components/settings/AutoSignIn.tsx` |

## Canvas authenticates by session cookie only

UniMelb Canvas has disabled self-service access tokens — only an admin can
issue one — so every request carries a snapshotted `canvas_session` cookie.
Do not propose `Authorization: Bearer`. (The "New Access Token" button's text
is on the page either way; only its `disabled` attribute tells you.)

- Sign-in goes through the university's SAML IdP, so the first login happens
  in a visible app WebView. The cookie is persisted to the data dir in a
  plaintext file only this user can read (`paths::write_private`, like every
  session file here), beside an auth-flag file meaning "we believe we have a
  session".
- The session has no cookie-side expiry: the server extends it on use, so
  periodic requests keep it alive. There is no remember-me cookie — once dead,
  it is rebuilt by [automated sign-in](#okta-sign-in-runs-headless-in-rust)
  or by hand.
- On launch with the flag present, the app starts connected while a thread
  probes the cookie: `Valid` confirms, `Rejected` clears the flag (keeping the
  Okta snapshot) and emits `canvas-auth-expired`, `Unreachable` stays connected.
- Signing out (Settings or `oculus auth logout`, both `paths::sign_out`)
  deletes the Canvas, Okta and Ed snapshots and the flag, and writes
  `canvas-session/signed-out`. The Okta one goes too, or the in-app browser
  would pass through SSO and re-save a Canvas session. The
  [attempt record](#every-sign-in-attempt-goes-through-one-guard) stays, so a
  lockout pause outlives a sign-out.
- The marker keeps every automatic sign-in off (`LoginError::SignedOut`)
  until a session is established again: `paths::mark_authenticated` removes
  it.
- Settings' sign-out first deletes every cookie WebKit would send to Canvas
  or Okta (`browser::clear_sessions`), or a tab still signed in would save the
  session straight back. `oculus auth logout` cannot reach a running app's
  jar, so a Canvas tab open there can still re-save the cookie.

## The in-app browser is seeded with the same sessions

`canvas_session` is HttpOnly and session-scoped, so WebKit keeps it in memory
only and the on-disk snapshot is the sole surviving copy. Okta's cookies for
`sso.unimelb.edu.au` are snapshotted beside it (`sso-session.cookie`, written
by a headless sign-in and by the browser), so a page that redirects to SSO
passes straight through.

- `browser::seed_sessions` writes both into WebKit's shared jar through
  `WKHTTPCookieStore` (Tauri has no cookie setter) at startup and before any
  `*.unimelb.edu.au` tab, navigation or reload, re-scoping every bare
  `name=value` pair to its host. The load waits for the write to land.
- Canvas cookies always replace the jar's; Okta's only fill gaps, unless the
  snapshot changed since it was last seeded — while the app runs, the
  browser's own Okta session is the freshest.
- A page that lands on Okta's entry for any SAML app (`/app/…/sso/saml`, the
  sign-in form when Okta has no session) asks `/api/v1/sessions/me` with the
  browser's Okta cookies. On a 404 it runs the
  [headless sign-in](#okta-sign-in-runs-headless-in-rust), seeds the Okta
  session it saved and reloads the page, subject to the
  [attempt guard](#every-sign-in-attempt-goes-through-one-guard).
- A signed-in Canvas page (`auth::is_authenticated_url`) re-snapshots both,
  deduped by name, so the scraper inherits the fresher cookie. A signed-out
  page never does: it would save Canvas's anonymous cookie over the good one.
- If the app is not connected when that happens, someone signed in by hand in
  the tab. `auth::confirm_browser_sign_in` checks the saved cookie with Canvas
  and, if Canvas accepts it, connects the app.

## Every sign-in ends in one place

The login window, a browser tab and the headless flow each put their cookies
on disk their own way, then call `auth::session_established`. It sets the
auth flag (which also removes the signed-out marker), the in-memory state and
`canvas-auth-success`. A sign-in by a person (window or tab) also clears the
attempt guard's wait and pause; the headless one has already updated the
guard. The CLI has no app to update, so `oculus auth auto` sets only the flag
(`paths::mark_authenticated`).

## Keep-alive runs in two layers

- **App open** — a thread in `app/src-tauri/src/lib.rs` re-probes every 6
  hours and emits `canvas-auth-expired` on rejection.
- **App closed** — `app/src-tauri/src/keepalive.rs` installs a LaunchAgent
  that runs `oculus auth tick`, which shares the app's probe, cookie merge and
  sign-in. Every outcome is a line in `session-keepalive.log` and exit 0,
  because launchd has no console and a non-zero exit reads as a crashed job.
- The tick re-authenticates only on `Rejected`; on `Unreachable` it logs and
  waits rather than spending an Okta attempt on a dead network. After a
  sign-out it logs and does nothing.
- The agent installs itself only after a headless sign-in has succeeded
  (`keepalive::ensure_installed`), not when credentials are stored: an org
  that offers only push or WebAuthn would fail every six hours forever.
  Turning it off writes a `keepalive-disabled` marker so the next sign-in
  does not reinstall it.
- The CLI is bundled into `Contents/MacOS/` beside the app. It is listed under
  `externalBin` in `tauri.conf.json` ("external" means prebuilt, not left out
  of the bundle), and `app/scripts/stage-cli.mjs` builds and stages it in
  `binaries/`. `tauri-build` validates every `externalBin`
  path even while building `oculus` itself, so the script writes an empty
  placeholder for that build and removes it on failure.
- The plist stores the CLI path absolutely; `keepalive::repair_path` re-points
  it on startup when the bundle has moved.
- Neither layer beats an absolute session cap or a forced IdP re-auth.

## Okta sign-in runs headless in Rust

`app/src-tauri/src/okta.rs` rebuilds a dead session without a browser. The
IdP is Okta Identity Engine at `sso.unimelb.edu.au`, a JSON state machine at
`/idp/idx/*`: introspect the login page's state token, answer each
*remediation*, then replay the SAML app URL and POST the assertion to Canvas.

- The loop dispatches on remediation **names**, not a fixed order, because
  factor order is an Okta policy setting.
- It answers **password** (macOS keychain) and **TOTP** (generated in-tree
  from a stored seed, pinned by the RFC 4226/6238 vectors). Okta Verify push
  and WebAuthn need a human; `LoginError::UnsupportedFactor` names the factors
  Okta did offer.
- Both probe sites in `lib.rs` call `okta::try_auto_recover` before declaring
  a session expired, and `useAuth().connect()` tries it before opening the
  login window. Credentials come from Settings → Canvas or `oculus auth setup`.
- A keychain read that is refused is `LoginError::UnreadableCredentials`,
  never `NotConfigured`: no attempt is made or recorded, `try_auto_recover`
  logs it, and Settings → Canvas shows it.
- Password and TOTP seed share one keychain, so against code already running
  as this user the second factor is not a second factor — the same posture as
  a password manager that stores TOTP.

## Every sign-in attempt goes through one guard

The startup probe, the in-app 6 h thread, the browser and `oculus auth tick`
each sign in on their own, in two processes, and Okta locks the account after
too many attempts. So `okta::sign_in` checks one record,
`canvas-session/sign-in.json`, under a file lock before each attempt, and logs
every attempt with its caller to `okta-sign-in.log`.

- An automatic attempt waits 10 min after any attempt, then 1 h after two
  failures in a row and 6 h after three. A success resets the count.
- A network failure waits but does not count: Okta gave no verdict.
- A lockout, a rejected password or a factor it cannot answer pauses automatic
  sign-in until a manual one succeeds (`Trigger::Manual`: Connect, saving
  credentials, `oculus auth auto`) or a person signs in in the login window
  or a browser tab. Saving
  credentials also clears the pause.
- Manual attempts skip the wait, because a person is waiting on the answer,
  but are still recorded.
- After a sign-out no automatic attempt starts at all, and none is recorded
  (`LoginError::SignedOut`).

## Ed mints its `x-token` from Canvas

Ed has no third-party OAuth; every API call carries the web app's `x-token`
JWT. `app/src-tauri/src/ed.rs` mints it by walking the Canvas → Ed LTI 1.3
launch (tool page → `oidc_login` → Canvas `/api/lti/authorize` → `launch` →
one-shot `?_logintoken=` → `POST /api/login_token`). Tokens last about two
weeks, are renewed with `POST /api/renew_token` on each sync, and a dead one
is re-minted. The token file is private to this user, like the cookies.
`oculus auth ed <TOKEN>` is a manual override.

## Echo360 has no stored credential

Each session is minted on demand from the Canvas cookie by POSTing the
course's LTI external-tool form (see
[sync.md](./sync.md#echo360-is-an-lti-launch-with-up-to-two-streams)), and
`app/src-tauri/src/lectures.rs` caches it per course.

## Gotchas

- Every path that establishes a session must end in `auth::session_established` (the CLI: `paths::mark_authenticated`) — the startup probe reads the flag first, and without it a valid cookie opens disconnected.
- Holding a `canvas_session` is not success: Canvas issues an anonymous one before auth, so `complete_saml` accepts one only after posting an assertion and `sign_in` verifies it against `/api/v1/users/self` before writing it.
- Answer `currentAuthenticator`'s challenge before reading the chooser — OIE offers `select-authenticator-authenticate` beside every challenge, and taking it loops forever.
- The login page names `stateToken` several times; the first is a fragment introspect rejects as "session has expired", so `state_token_candidates` tries every plausible one.
- Only `/idp/idx/introspect` takes `stateToken`; every later call sends `stateHandle`.
- A rejected password is cleared and automatic sign-in paused, or every automatic path would replay it until Okta locks the account.
- A TOTP seed cannot be recovered from codes; getting it means re-enrolling the factor.
- `setCookies:completionHandler:` must get a real block — nil segfaults the app seconds later from a WebKit-only stack.
- WebKit drops an API-set cookie that would replace an HttpOnly one a server set, with no error. Once Canvas hands the browser an anonymous `canvas_session`, re-seeding does nothing until the old cookie is deleted — so seeding deletes same-named cookies first.
- `/login/session_token` answers 403 to a cookie session; it wants an access token, which is disabled here.
- Ed's LTI redirect walk jars cookies per host; the Canvas cookie must never reach edstem.org.

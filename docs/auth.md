# Auth & sessions

Three services, three credentials — all derived from one Canvas session
cookie.

## Where

| Piece | Location |
| --- | --- |
| Canvas sign-in, session persistence | `app/src-tauri/src/auth.rs` |
| Session probe (`Valid`/`Rejected`/`Unreachable`) | `app/src-tauri/src/canvas.rs` |
| Cookie, auth-flag and keep-alive log paths | `app/src-tauri/src/paths.rs` |
| Headless Okta sign-in, TOTP, stored credentials | `app/src-tauri/src/okta.rs` |
| LaunchAgent keep-alive (app closed) | `app/src-tauri/src/keepalive.rs` |
| The `auth tick` the agent runs | `app/src-tauri/src/bin/oculus/auth.rs` |
| Staging the CLI into the bundle | `app/scripts/stage-cli.mjs` |
| In-app keep-alive loop + startup probe | `app/src-tauri/src/lib.rs` |
| Ed token minting via LTI | `app/src-tauri/src/ed.rs` |
| Echo360 session via LTI, per-course cache | `app/src-tauri/src/echo360.rs`, `app/src-tauri/src/lectures.rs` |
| Frontend auth state | `app/src/hooks/useAuth.ts`, `app/src/hooks/useKeepalive.ts` |
| Handing the session to the in-app browser | `app/src-tauri/src/browser.rs` |
| Credential entry UI | `app/src/components/settings/AutoSignIn.tsx` |

## Canvas authenticates by session cookie only

UniMelb Canvas has disabled self-service access tokens — only an admin can
issue one — so every request carries a snapshotted `canvas_session` cookie.
Do not propose `Authorization: Bearer`. (The "New Access Token" button's text
is on the page either way; only its `disabled` attribute tells you.)

- Sign-in goes through the university's SAML IdP, so the first login happens
  in a visible app WebView. The cookie is persisted to the data dir in
  plaintext beside an auth-flag file meaning "we believe we have a session".
- The session has no cookie-side expiry: the server extends it on use, so
  periodic requests keep it alive. There is no remember-me cookie — once dead,
  it is rebuilt by [automated sign-in](#okta-sign-in-runs-headless-in-rust)
  or by hand.
- On launch with the flag present, the app starts connected while a thread
  probes the cookie: `Valid` confirms, `Rejected` clears the flag (keeping the
  SSO profile) and emits `canvas-auth-expired`, `Unreachable` stays connected.

## The in-app browser is seeded with the same cookie

`canvas_session` is HttpOnly and session-scoped, so WebKit keeps it in memory
only and the on-disk snapshot is the sole surviving copy.
`browser::seed_canvas_session` writes it into WebKit's shared jar through
`WKHTTPCookieStore` (Tauri has no cookie setter) at startup and before each
Canvas page, re-scoping every bare `name=value` pair to the Canvas host.
Browsing Canvas in-app rolls the session forward and re-snapshots it, deduped
by name, so the scraper inherits the fresher cookie.

## Keep-alive runs in two layers

- **App open** — a thread in `app/src-tauri/src/lib.rs` re-probes every 6
  hours and emits `canvas-auth-expired` on rejection.
- **App closed** — `app/src-tauri/src/keepalive.rs` installs a LaunchAgent
  that runs `oculus auth tick`, which shares the app's probe, cookie merge and
  sign-in. Every outcome is a line in `session-keepalive.log` and exit 0,
  because launchd has no console and a non-zero exit reads as a crashed job.
- The tick re-authenticates only on `Rejected`; on `Unreachable` it logs and
  waits rather than spending an Okta attempt on a dead network.
- The agent installs itself only after a headless sign-in has succeeded
  (`keepalive::ensure_installed`), not when credentials are stored: an org
  that offers only push or WebAuthn would fail every six hours forever.
  Turning it off writes a `keepalive-disabled` marker so the next sign-in
  does not reinstall it.
- The CLI ships as a Tauri `externalBin`, staged by
  `app/scripts/stage-cli.mjs`: the bundler copies some cargo bins into
  `Contents/MacOS/` on its own, but not `oculus`. `tauri-build` validates
  every `externalBin` path even while building `oculus` itself, so the script
  writes an empty placeholder for that build and removes it on failure.
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
- Password and TOTP seed share one keychain, so against code already running
  as this user the second factor is not a second factor — the same posture as
  a password manager that stores TOTP.

## Ed mints its `x-token` from Canvas

Ed has no third-party OAuth; every API call carries the web app's `x-token`
JWT. `app/src-tauri/src/ed.rs` mints it by walking the Canvas → Ed LTI 1.3
launch (tool page → `oidc_login` → Canvas `/api/lti/authorize` → `launch` →
one-shot `?_logintoken=` → `POST /api/login_token`). Tokens last about two
weeks, are renewed with `POST /api/renew_token` on each sync, and a dead one
is re-minted. `oculus auth ed <TOKEN>` is a manual override.

## Echo360 has no stored credential

Each session is minted on demand from the Canvas cookie by POSTing the
course's LTI external-tool form (see
[sync.md](./sync.md#echo360-is-an-lti-launch-with-up-to-two-streams)), and
`app/src-tauri/src/lectures.rs` caches it per course.

## Gotchas

- Every path that establishes a session must call `paths::mark_authenticated` — the startup probe reads the flag first, and without it a valid cookie opens disconnected.
- Holding a `canvas_session` is not success: Canvas issues an anonymous one before auth, so `complete_saml` accepts one only after posting an assertion and `sign_in` verifies it against `/api/v1/users/self` before writing it.
- Answer `currentAuthenticator`'s challenge before reading the chooser — OIE offers `select-authenticator-authenticate` beside every challenge, and taking it loops forever.
- The login page names `stateToken` several times; the first is a fragment introspect rejects as "session has expired", so `state_token_candidates` tries every plausible one.
- Only `/idp/idx/introspect` takes `stateToken`; every later call sends `stateHandle`.
- A rejected password is cleared, or the keep-alive replays it every six hours until Okta locks the account.
- A TOTP seed cannot be recovered from codes; getting it means re-enrolling the factor.
- `setCookies:completionHandler:` must get a real block — nil segfaults the app seconds later from a WebKit-only stack.
- `/login/session_token` answers 403 to a cookie session; it wants an access token, which is disabled here.
- Ed's LTI redirect walk jars cookies per host; the Canvas cookie must never reach edstem.org.

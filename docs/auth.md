# Auth & sessions

Three services, three credentials — all derived from one Canvas session
cookie, at whichever university the institution profile names. Two secrets
keyd keeps are not: Spotify's login, for the music player
([below](#spotifys-credentials-are-read-back-with-get)), and WebKit's
WebCrypto master key ([below](#webkits-webcrypto-master-key-comes-from-keyd-per-build)).

## Where

| Piece | Location |
| --- | --- |
| The app's side of the sign-in: login window, hand-over from a login window or browser tab, startup restore, sign-out, the CLI's sign-in watch | `app/src-tauri/src/auth/` |
| Canvas requests through keyd's `canvas` route; session probe (`Valid`/`Rejected`/`Unreachable`) | `app/src-tauri/src/sources/canvas/` |
| The app's Okta commands and calls: each goes to keyd; only the credential calls fall back to the keychain when keyd is absent | `app/src-tauri/src/auth/okta/` |
| The sign-in flow, TOTP and the attempt guard; it runs only inside keyd | `app/keyd/core/src/okta/` |
| keyd's Okta ops: the vault entries, the old-item import, one sign-in at a time; the role check every op passes | `app/keyd/core/src/ops/okta.rs`, `app/keyd/core/src/ops.rs` |
| Staging the CLI into the bundle | `app/scripts/stage-cli.mjs` |
| Startup probe | `app/src-tauri/src/auth/startup.rs` |
| Ed token minting via LTI | `app/src-tauri/src/sources/ed/lti.rs` |
| Echo360 session via LTI, per-course cache | `app/src-tauri/src/sources/echo360/auth.rs`, `app/src-tauri/src/lectures/mod.rs` |
| Echo360 page embeds: per-link launch, Bearer, CDN signing; the shared Bearer | `app/src-tauri/src/sources/echo360/embed/`, `app/src-tauri/src/sync/scrape/embeds/state.rs` |
| Frontend auth state | `app/src/hooks/sync/useAuth.ts` |
| Handing the session to the in-app browser | `app/src-tauri/src/shell/browser/seed.rs` |
| Keychain entry lifecycle (the fallback for the Okta credentials and for MinerU, Voyage and Groq when keyd is absent) | `app/src-tauri/src/providers/credentials/keychain.rs` (`Secret`), `app/src-tauri/src/providers/credentials/mod.rs` |
| keyd's login sessions: the vault entries, the cookie rule shared with the sign-in's jar, the two markers, the `session_*` ops and `sign_out`, the one-time import of the old session files | `app/keyd/core/src/session/`, `app/keyd/core/src/ops/session.rs`, `app/keyd/core/src/ops/legacy.rs` |
| The `canvas` and `ed` routes of `forward` (cookie and `x-token` attached by keyd), its path rules and streaming; the Canvas route's sign-in on a rejected request | `app/keyd/core/src/forward/`, `app/keyd/core/src/ops/forward.rs` ([architecture.md](./architecture.md#oculus-keyd-is-the-only-process-meant-to-read-its-key)) |
| The `oculus-keyd` client, which the Voyage, MinerU and Groq keys and the Okta credentials and sign-in go through when keyd is installed | `app/keyd/core/src/client.rs` (`credentials::Credentialed`), `app/src-tauri/src/providers/credentials/mod.rs` (`CloudKey`) |
| Credential entry UI (Settings → Canvas, and onboarding's *Never sign in again*) | `app/src/components/settings/web/AutoSignIn.tsx`, `app/src/components/onboarding/canvas/NeverSignInAgain.tsx` |
| Spotify's stored credentials: the `music.spotify` vault entry, keyd's app-only `get`, and the music module that writes and reads it | `app/keyd/core/src/names.rs` (`READABLE`), `app/keyd/core/src/ops/get.rs`, `app/src-tauri/src/music/credentials.rs` ([music.md](./music.md)) |
| WebKit's WebCrypto master key: the per-build names and items, the key's text form, keyd's `webcrypto_key` and its import, the client call | `app/keyd/core/src/names.rs` (`WEBCRYPTO`), `app/keyd/core/src/webcrypto.rs`, `app/keyd/core/src/ops/webcrypto.rs`, `app/keyd/core/src/client/webcrypto.rs`, the app's reader `app/src-tauri/src/shell/webcrypto/mod.rs` (`fetch`) |
| Handing the key to WebKit: the data-store delegate, its once-per-process cache, where it is installed | `app/src-tauri/src/shell/webcrypto/delegate.rs`, `app/src-tauri/src/shell/webcrypto/cache.rs`, `install_default` in `app/src-tauri/src/shell/webcrypto/mod.rs` (called first in `app/src-tauri/src/lib.rs`'s setup), `configure_store` in `app/src-tauri/src/shell/dock/page.rs` |
| Institution profile: the Canvas address, sign-in detection, the UniMelb migration, telling keyd | `app/src-tauri/src/sources/institution/` |
| The site keyd sends the session to, and `site_set` | `app/keyd/core/src/site.rs`, `app/keyd/core/src/ops/site.rs` |
| Canvas address UI, the profile on the frontend | `app/src/components/settings/web/CanvasAddress.tsx`, `app/src/stores/sync/institutionStore.ts` |
| The guard that no university or single region is named in app code | `app/tests/repo/uniAgnostic.test.ts` |
| Start over | `app/src-tauri/src/library/storage/start_over.rs`, `app/src/components/settings/library/StartOver.tsx` |

## Oculus works at any university

Every student types their own Canvas address, so nothing in app code may
assume which university, or which region of a service, they are at.

- **What differs per university comes from the profile**
  ([below](#one-institution-profile-says-which-university)): Rust through
  `sources::institution` (`canvas_base`, `current`, `load`), the frontend
  through `institutionStore`. A link the frontend builds before the profile
  has loaded is resolved when clicked (`canvasFileLink` in
  `app/src/lib/files/canvasLinks.ts`).
- **What a launch reveals is learned, then saved to the profile**: Ed's API
  host and web region from the page its LTI launch lands on, the Echo360
  tool from the course's tabs. A feature that needs one before any launch
  says so in its error rather than guessing a region.
- **A regional service is checked against all of its regions**, never one:
  Echo360's hosts are listed in `app/src-tauri/src/sources/echo360/embed/region.rs`, and
  the calls after a launch go to the host it landed on.
- **UniMelb appears only where an install from before the profile needs it**:
  `institution::unimelb()`, keyd's `Site::unimelb()` and tests.
  `app/tests/repo/uniAgnostic.test.ts` scans app code (comments, tests and
  fixtures aside) for a university's domain or a single Echo360 or Ed region
  and fails `bun run test` on a hit outside those files.

## One institution profile says which university

`institution.json` in the data dir holds the Canvas address (`https://host`),
the path the sign-in window opens, the IdP type (`okta` or `other`) and its
host, and what launches have learned: the Echo360 tool fallback and lead-in,
Ed's API base and web region, course-code prefixes that are never subjects. It is a file, not
a `settings` row, because synchronous code needs the Canvas host without a
database pool; reads are cached on the file's mtime, so the CLI and the app see
each other's writes.

- **The student types one thing**, their Canvas address, in Settings → Canvas
  or `oculus auth canvas <address>`. `institution::configure` keeps only the
  origin, proves it is Canvas (an anonymous `/api/v1/users/self` answers 401
  `{"status":"unauthenticated"}`), and walks the sign-in redirects with no
  session (`keyd_core::okta::probe_sign_in_page`): `/login` first, then
  `/login/saml`, stopping at the first that reaches Okta's widget. Where the
  walk leaves Canvas is the SSO host.
- **An install older than the profile is UniMelb's.** A missing file beside an
  old cookie file, keyd's authenticated flag or signed-out marker, an attempt
  record or a non-empty `courses/` gets the UniMelb profile written
  (`institution::unimelb`). A fresh install has none of those (the database
  alone is no evidence — the app creates it before anyone signs in), so Canvas
  stays unset until the student enters an address.
- **The address cannot change under a library.** Subject rows are keyed by
  Canvas course id, which another Canvas reuses, so a new address is refused
  while `subjects` has rows. Start over (Settings → Storage) signs out,
  forgets the Okta credentials, leaves a `start-over` marker and restarts; the
  next launch empties the data dir before any thread or database handle
  exists, keeping the two cloud usage files and keyd's vault, socket and
  `bin/`. The frontend clears its library-scoped `localStorage` keys first.
- Saving a different address signs out (`auth::commands::end_session`, while
  the old hosts are still the profile's, so the browser jar loses the old
  session) and clears the Okta credentials, which belong to the old IdP.

## keyd sends the session only to the profile's university

keyd holds its own copy of the origins it may send to, the *site*
(`keyd_core::site::Site`: Canvas, the login path, Okta's origin when the IdP is
Okta, Ed), sealed in the vault as `keyd.site`, so no file another process can
edit decides where a cookie goes. The `canvas` and `ed` routes and the sign-in
read it; a vault without one is UniMelb's.

- The app sets it with `site_set` (app and CLI only) from the profile
  (`Institution::site`): before saving a new address, when a launch learns
  Ed's API host, and at startup before the session is restored. keyd hears
  first, so a refusal leaves the profile unchanged; with keyd absent there is
  no session to send anywhere.
- **Moving Canvas's or Okta's origin is moving university**: under the
  sign-in's single flight, keyd drops all three sessions, the authenticated
  flag and the Okta credentials (and their old keychain items). Moving only
  Ed's origin drops only Ed's token. A cookie or password is never sent to an
  origin other than the one it was made for.
- `site_set` refuses an origin that is not a bare `https://host` and a login
  path that is not a plain path on Canvas.
- A site with no Okta origin answers every sign-in with `LoginError::NotOkta`.


## Canvas authenticates by session cookie only

Universities commonly disable students' self-service access tokens (UniMelb
does — only an admin can issue one), so every request carries a snapshotted
`canvas_session` cookie, everywhere. Do not propose `Authorization: Bearer`.
(The "New Access Token" button's text is on the page either way; only its
`disabled` attribute tells you.)

- Sign-in goes through whatever the university uses — SAML, CAS, OIDC or
  Canvas's own form — so the first login happens in a visible app WebView
  opened at the profile's login path. It waits only for a signed-in Canvas URL
  on the profile's host, so it is the same for every IdP. The session lives in
  keyd's vault ([below](#with-keyd-installed-keyd-holds-the-login-sessions)):
  the headless sign-in stores it there itself, and the login window and
  browser tabs read their webview's Canvas and SSO cookies and hand them to
  keyd with `session_put` (`auth::save_session_cookie`), skipping any host with
  no cookie yet and logging a refused put. Capture reads the whole jar and
  selects cookies for each host, including parent domains; same-named cookies
  prefer a server cookie over an unchanged host copy written by seeding, then
  the more specific domain. Wry's macOS `cookies_for_url` only matches
  the exact domain and can omit cookies the browser sends. An authenticated
  flag, which keyd owns, means "we believe we have a session".
- The session has no cookie-side expiry: the server extends it on use. There
  is no remember-me cookie — once dead, it is rebuilt by
  [automated sign-in](#okta-sign-in-runs-headless-in-rust) or by hand.
- On launch a thread asks keyd for the flag; with it present the app starts
  connected while the thread probes the session: `Valid` confirms, `Rejected`
  clears the flag through `session_mark(false)` (keeping the SSO session) and
  emits `canvas-auth-expired`, `Unreachable` (no answer, a Canvas 5xx, or keyd
  not running) stays connected. A keyd that cannot be asked reads as not
  signed in, logged once. `get_auth_status` is the flag or this run's
  in-memory state, read off the main thread.
- Signing out (Settings or `oculus auth logout`) is keyd's `sign_out`: it
  deletes the Canvas, SSO and Ed sessions and the flag, and writes
  `canvas-session/signed-out`. The SSO one goes too, or the in-app browser
  would pass through SSO and re-save a Canvas session. The app's sign-out
  (`auth::commands::end_session`) emits `canvas-signed-out` for every
  `useAuth`. The
  [attempt record](#every-sign-in-attempt-goes-through-one-guard) stays, so a
  lockout pause outlives a sign-out. With keyd absent there is nothing to
  drop.
- The marker keeps every automatic sign-in off (`LoginError::SignedOut`)
  until a session is established again: `session_mark(true)` removes it, and
  so does every sign-in keyd runs.
- Settings' sign-out first deletes every cookie WebKit would send to Canvas
  or the SSO host (`shell::browser::clear_sessions`), or a tab still signed in
  would save the session straight back. `oculus auth logout` cannot reach a running app's
  jar, so a Canvas tab open there can still re-save the cookie.

## The in-app browser is seeded with the same sessions

`canvas_session` is HttpOnly and session-scoped, so WebKit keeps it in memory
only and keyd's copy is the one that survives a restart. The SSO host's
cookies are kept beside it (written by a headless sign-in and by the browser),
so a page that redirects to SSO passes straight through.

- `shell::browser::seed_sessions` asks keyd for both (`session_get`, app only) on a
  thread of its own, because the first call after an update can wait on the
  keychain prompt, then writes them into WebKit's shared jar through
  `WKHTTPCookieStore` (Tauri has no cookie setter) at startup and before any
  tab, navigation or reload on the university's hosts, re-scoping every bare
  `name=value` pair to its host. The load waits for the write to land. With
  keyd absent, or holding no session, it seeds nothing and the load goes on.
- The university's hosts are the Canvas host, the SSO host, and the domain
  they share when it is the university's own (`institution_domain`:
  `unimelb.edu.au`, never a public suffix like `edu.au` or a shared host like
  `instructure.com`).
- Canvas cookies always replace the jar's; the SSO host's only fill gaps,
  unless keyd's header differs from the last one seeded this run (compared by
  hash) — while the app runs, the browser's own SSO session is the freshest.
- At an Okta university, a page that lands on Okta's entry for any SAML app
  (`/app/…/sso/saml`, the sign-in form when Okta has no session) asks
  `/api/v1/sessions/me` with the browser's Okta cookies. On a 404 it runs the
  [headless sign-in](#okta-sign-in-runs-headless-in-rust), seeds the Okta
  session it saved and reloads the page, subject to the
  [attempt guard](#every-sign-in-attempt-goes-through-one-guard).
- A signed-in Canvas page (`auth::is_authenticated_url`) hands both cookie
  sets to keyd, deduped by name, so the scraper inherits the fresher cookie. A
  signed-out page never does: it would store Canvas's anonymous cookie over the
  good one. The jar is read on the page-load callback and the keyd calls run on
  another thread.
- If the app is not connected when that happens, someone signed in by hand in
  the tab. `auth::tab::confirm_browser_sign_in` checks the stored session with
  Canvas and, if Canvas accepts it, connects the app.

## Every sign-in ends in one place

The login window, a browser tab and the headless flow each store their session
their own way, then use the shared recording and announcement in `auth`.
Tabs and headless flows call `auth::session_established`; the window separates
the blocking recording from the announcement so closing it can cancel a
pending confirmation. For a person's sign-in
(window or tab) it sets the flag with `session_mark(true)`, which also removes
the signed-out marker; if keyd cannot record that, the app is not signed in
and the UI gets `canvas-auth-cancelled`. It then sets the in-memory state and
emits `canvas-auth-success`. A sign-in by a person also clears the attempt
guard's failures, pause and wait, by asking keyd (`okta_resume`, app only);
the app writes the record itself only when keyd is absent. The headless
sign-in has already updated the guard, and keyd has already set the flag, for
every trigger, so `oculus auth auto` sets nothing. The login window captures
cookies after a signed-in Canvas page finishes loading and reports success
only once the stored session passes the Canvas API probe. A failed capture or
probe leaves the window open, and another completed Canvas load can retry
(including a load that arrived while the probe ran);
closing the window while confirmation runs cancels it.

`oculus auth login` opens the app and waits for a sign-in it did not make
itself. The signal is keyd's `session_status`: a sign-in is new when Canvas is
held and the flag is on, and either was not true at the start or `generation`
differs from the last reading (`auth::signed_in_since`). `generation` restarts
when keyd does, so a restart that lands on the same number is caught by the
flag flipping on. A Canvas cookie rotating also changes `generation`, so the
command confirms with Canvas before reporting success.

## A dead session is rebuilt when something needs it

Nothing pings Canvas on a timer and no LaunchAgent signs in for the app.
Three things start an automatic sign-in, each through the
[attempt guard](#every-sign-in-attempt-goes-through-one-guard):

- **A Canvas request keyd finds rejected.** A `canvas` request whose
  session is missing, whose answer is a 401, or whose answer is a redirect
  to the SSO host or to Canvas's `/login`, makes keyd sign in
  (`Trigger::Forward`) and send the request once more; see
  [below](#a-rejected-canvas-request-signs-in-once-and-is-sent-once-more).
  Ed's route never does: its token comes from a Canvas launch only the app
  performs, so an Ed 401 is returned as it is.
- **The app's startup probe**, when the saved session is rejected.
- **A browser tab** that lands on Okta's entry for a SAML app.

`oculus auth auto` and Settings → Canvas → Connect are manual attempts.

A sign-in only works for an account whose factors are a password and TOTP;
an org that offers only push or WebAuthn pauses automatic sign-in at the first
attempt (`LoginError::UnsupportedFactor`).

- The CLI is bundled into `Contents/MacOS/` beside the app, because keyd's
  caller check admits only executables in the app its helper is nested in. It is listed under
  `externalBin` in `tauri.conf.json` ("external" means prebuilt, not left out
  of the bundle), and `app/scripts/stage-cli.mjs` builds and stages it in
  `binaries/`. `tauri-build` validates every `externalBin`
  path even while building `oculus` itself, so the script writes an empty
  placeholder for that build and removes it on failure.
- No sign-in beats an absolute session cap or a forced IdP re-auth.

## Okta sign-in runs headless in Rust

`keyd_core::okta::sign_in` (`app/keyd/core/src/okta/`) rebuilds a dead session
without a browser, inside keyd, at a university whose site has an Okta origin.
Okta Identity Engine is a JSON state machine at `/idp/idx/*` on that origin:
introspect the login page's state token, answer each *remediation*, then
replay the SAML app URL and POST the assertion to Canvas.

- Anywhere else keyd answers `LoginError::NotOkta` before it reads the
  credentials or the attempt record, `try_auto_recover` treats it like "never
  set up", and Settings → Canvas does not offer automatic sign-in. Those
  students sign in in the window when a session dies.
- The loop dispatches on remediation **names**, not a fixed order, because
  factor order is an Okta policy setting.
- It answers **password** and **TOTP** (generated in-tree from a stored
  seed, pinned by the RFC 4226/6238 vectors), both from keyd's vault. Okta
  Verify push
  and WebAuthn need a human; `LoginError::UnsupportedFactor` names the factors
  Okta did offer.
- The startup probe in `auth/startup.rs` calls `auth::okta::try_auto_recover` before declaring
  a session expired, and `useAuth().connect()` tries it before opening the
  login window. Credentials come from Settings → Canvas or `oculus auth setup`.
- A credential read that is refused (keyd's master key, or an old keychain
  item) is `LoginError::UnreadableCredentials`, never `NotConfigured`: no
  attempt is made or recorded, `try_auto_recover` logs it, and Settings →
  Canvas shows it.
- Password and TOTP seed are kept together, so against code already running
  as this user the second factor is not a second factor — the same posture as
  a password manager that stores TOTP.

## With keyd installed, keyd holds the credentials and runs the sign-in

`auth/okta/` in the app only routes. Saving, forgetting, the Settings status and
every sign-in a client starts (Settings, the startup probe, the browser,
`oculus auth auto`) are requests to `oculus-keyd` through
`credentials::Credentialed` (ops `okta_save`, `okta_forget`, `okta_status`,
`ensure_signed_in`, and `okta_resume` for the guard). Neither process reads the password or the seed back: no
op returns one (keyd's read-backs are `get`, which serves only Spotify's
credentials, [below](#spotifys-credentials-are-read-back-with-get), and
`webcrypto_key`, which serves only WebKit's master key),
and `oculus auth setup` prints the code from the seed just typed.

- **Only `KeydError::Absent` takes the old route, and only for the
  credentials.** The keychain items (`com.oculus.unimelb-sso`: `username`,
  `password`, `totp_secret`) answer status, save and forget only when nothing
  is listening on `keyd.sock`. A sign-in with keyd absent is
  `LoginError::Broker` ("oculus-keyd is not running …"): the session it would
  mint is one only keyd can use. Every other keyd error surfaces and starts
  no second route.
- **Failures map to the sign-in's own errors.** `ensure_signed_in` carries a
  failed sign-in as its `LoginError`, variant for variant, so callers act on
  it as they do in-process. A `keychain` error is
  `LoginError::UnreadableCredentials` (macOS refused either this program or
  keyd's master key); any other keyd error is `LoginError::Broker` with the
  client's description of it ("The sign-in request to oculus-keyd failed: …"),
  which is not a sign-in step.
- **keyd validates a save.** The app sends the values as typed, and
  `validate_credentials` (the check the keychain route applies too) runs in
  keyd; its message for bad input reaches the user unchanged, and for a bad
  setup key never repeats any of it. A save also clears the attempt guard's
  failures, pause and wait.
- **The setup key may be pasted as a key or as a link.** `okta::setup_key`
  (`okta/setup_key.rs`) takes Okta's spaced, lowercase key or a whole
  `otpauth://` link (any case of the scheme), keeping only the link's
  percent-decoded `secret=`; a link without one is a bad key. The vault holds
  the bare key either way. Google Authenticator's `otpauth-migration://`
  export link is refused with its own message, since it holds no single key.
  `totp_now` takes the key as pasted too, so `oculus auth setup` prints the
  right code from a link.
- **The vault holds `okta.username`, `okta.password` and `okta.totp_secret`.**
  The first `okta_status` or sign-in copies the old keychain items in, one
  keychain prompt each, once; the items stay in the keychain after an import,
  and a save or a forget marks all three imported, so an old item is never
  copied back over either. `okta_forget` also deletes the three old items
  (best effort), so the keychain fallback cannot sign in with a forgotten
  login; it replies `legacy`: `removed`, `absent`, `failed` or `refused`.
- **The app and the CLI only.** keyd's caller check admits any executable
  in the install, so every op but `ping` (not just the Okta ones) also
  requires the `app` or `cli` role; ffmpeg, which ships in the bundle, is
  refused. The check is `ops::require_role`, called once by `dispatch`.
  `okta_resume` is the app's alone, because it lifts a lockout pause.
- **One sign-in runs at a time.** A request that arrives while keyd is
  signing in waits for that attempt and returns its outcome, rather than
  starting another or being held off by the guard. The socket has no timeout
  on this op: the flow can wait for the next TOTP window.
- **The sessions a sign-in mints go to keyd's vault**, through the `SessionStore`
  the flow is given (`okta::Env`), after Canvas has accepted the cookie. A
  success sets the authenticated flag, whatever the trigger. No reply carries a
  password or a seed, and none a cookie but `session_get`'s
  ([next](#with-keyd-installed-keyd-holds-the-login-sessions)).

## With keyd installed, keyd holds the login sessions

The vault holds three more entries: `session.canvas` and `session.sso`, each a
`name=value; …` cookie header, and `session.ed`, Ed's `x-token`. They are
filled by the sign-in, by `session_put` and by Canvas's own `Set-Cookie`
rotation, and read by `forward`, `session_get` and `session_status`.

- **A session is used through `forward`, never read out.** The `canvas` route
  sends `Cookie: <session.canvas>` and the `ed` route `x-token:
  <session.ed>`; a client cannot set `Cookie`, `X-Token` or `Authorization`
  itself, and neither route's reply carries `set-cookie`, `authorization` or
  `x-token`. With no session stored, `forward` answers `missing`.
  The routes, paths and streaming are in
  [architecture.md](./architecture.md#oculus-keyd-is-the-only-process-meant-to-read-its-key).
- **Cookies leave keyd only through `session_get`, to the app.** It returns
  `{"canvas", "sso"}` (a string or null each, never Ed's token) so the app can
  seed the in-app browser's cookie store; any other role gets `caller`.
  `session_put {kind, value}` (`canvas`, `sso` or `ed`; app or CLI) replaces a
  whole session, as a browser sign-in snapshot or a pasted Ed token does.
  `session_clear {kinds?}` (app or CLI; every kind by default) removes sessions
  and does not touch the markers or `sign-in.json`. `session_status` reports
  `canvas`, `sso`, `ed`, `authenticated` and `signed_out` as booleans, and
  `generation` as a number.
- **The generic ops never write a session.** `store` and `delete` refuse
  `session.*` as they refuse `okta.*`; `has` reports presence.
- **A value is at most 24 KiB of printable ASCII.** It rides in the request's
  header line (64 KiB, JSON-escaped); the client checks before sending and
  keyd checks again.
- **Canvas's `Set-Cookie` keeps the session current.** Every answer from the
  `canvas` route, a redirect included, is merged into `session.canvas` by
  `session::cookie::merge_set_cookie`, under the vault's lock. Only the leading
  `name=value` is read: a rotated cookie keeps its place, a new one goes last,
  and one with an empty value, a `Max-Age` of 0 or less, or (without
  `Max-Age`) an `Expires` in the past is removed. A cleared session is not
  recreated by a late answer, and a merge past the size limit is dropped. The
  sign-in's jar (`okta/jar.rs`) applies the same rule per host. Ed's
  `Set-Cookie` is ignored.
- **A generation counter** in keyd's memory counts every put, clear and
  absorbed change (`State::session_generation`); `session_status` reports it,
  and it restarts at 0 when keyd exits idle.
- **Keyd owns the two markers**, in `canvas-session/`: `authenticated` ("a
  session Canvas accepted is held"; the startup probe reads it) and
  `signed-out` (every automatic sign-in stands down). Every sign-in keyd runs
  sets the first and, if it was manual, lifts the second. `session_mark
  {authenticated}` (app or CLI) is for a person's sign-in in the login window,
  a tab or the CLI: `true` sets the flag and lifts `signed-out`, `false` only
  clears the flag. `sign_out {}` (app or CLI) clears all three sessions and the
  flag, writes `signed-out` and keeps `sign-in.json`, so a lockout pause
  outlives it; it replies `{"had": bool}`. It waits for a sign-in already
  running, which would otherwise save a session and lift `signed-out` after it;
  a sign-in requested meanwhile gets `signed_out`.
- **The old session files are imported once.** The first session op,
  `canvas` or `ed` forward or sign-in after keyd starts moves
  `canvas-session.cookie`, `sso-session.cookie` and `ed-session.token` into the
  vault, unless it already holds that session, and deletes each file
  (`keyd.imported.session.<kind>` records it). `session_clear` and `sign_out`
  import first, so a cleared session is never brought back from an old file.

## Spotify's credentials are read back with `get`

librespot logs in to Spotify with a stored-credentials blob it must hold
itself; there is no HTTP request keyd could attach it to, as `forward` does
for a key or a cookie. So the vault's `music.spotify` entry (username, blob,
Connect device id, as JSON) has a read-back of its own.

- **`get {secret}` returns `{"value": string|null}`, to the app only.** Every
  other role gets `caller`, from `require_role`, before the vault is opened.
- **Only the names in `names::READABLE` can be read**, and that list is
  `music.spotify` alone. Any other name, a cloud key, an Okta value or a
  session included, is `request`; a test pins the list.
- **The generic `store` and `delete` write it** (app or CLI), with no old
  keychain item to import. Okta's `okta_forget`, `sign_out` and `session_clear`
  leave it alone, so signing out of Canvas keeps Spotify signed in.
- With keyd absent there is no keychain fallback for it: the music module
  reports the error and keeps nothing ([music.md](./music.md#the-credentials-live-in-keyd-read-back-by-an-app-only-get)).

## WebKit's WebCrypto master key comes from keyd, per build

WebKit wraps every `CryptoKey` a page saves to IndexedDB (WhatsApp's and
Messenger's logins among them) with one 16-byte master key, which it keeps in
a login-keychain item of its own. An ad-hoc signed dev build is a new program
to that item's access list after every rebuild, so its prompt comes back each
time. keyd keeps a copy of the key and serves it to the app, and the app
hands it to WebKit through each data store's delegate, so WebKit never reads
its item.

- **One key per WebKit application.** WebKit files the key under the app's
  bundle identifier, or else its executable's name: account
  `com.apple.WebKit.WebCrypto.master+app` for the dev build and
  `…+com.tchan.oculus` for the bundle, a generic password with no service.
  Both builds share one vault, so each has its own entry,
  `webkit.webcrypto.app` and `webkit.webcrypto.com.tchan.oculus`
  (`names::WEBCRYPTO`).
- **`webcrypto_key {app}` replies `{"key", "source"}`, to the app only**, and
  only for the application the caller is: the peer check names it
  (`Caller::application`: the bundle's `CFBundleIdentifier`, or the dev
  executable's file name), and any other `app` is `caller`. A wrong key fails
  silently (IndexedDB returns `undefined` and those services sign out), so
  neither build can be handed the other's. An `app` with no entry is `request`.
- **The key travels as text**: the padded standard base64 of the 16 bytes,
  24 characters, as WebKit's item holds it (`crate::webcrypto`). keyd stores
  and sends exactly that; the client decodes it to 16 bytes. A value that does
  not decode to exactly 16 bytes is never served, stored or replaced.
- **The first request copies WebKit's item in.** `source` says where the key
  came from: `vault`, `keychain` (WebKit's item, read as WebKit reads it, by
  account alone, and left in place, never changed) or `minted` (16 fresh
  bytes). keyd makes a key only when the keychain says no such item exists; a
  refused prompt, a denied read or any other failure is `keychain`, stores
  nothing, and is asked again next time. Racing first requests wait on one
  read (the import lock) and get the same key; the vault keeps whichever key
  it held first.
- **The generic ops never touch it.** `store` and `delete` refuse the two
  names, `get` does not read them back, and `sign_out` leaves them. `has`
  reports presence.
- **Any error leaves the key to WebKit.** Whatever `Client::webcrypto_key`
  returns other than a key (`Absent`, `Keychain`, `Caller`, …), the caller
  hands WebKit nothing and WebKit reads its own item. The app's reader,
  `shell::webcrypto::fetch`, names the application as WebKit does (the main
  bundle's identifier, else the process name), returns the 16 bytes or `None`,
  and logs a failure or a first copy with `elog!`, never the key. It blocks on
  keyd, which can wait on a keychain prompt, so it runs off the main thread.
- **The app hands WebKit the key through a website data store delegate.**
  Before it reads its keychain item, WebKit asks the store's delegate (private
  `-[WKWebsiteDataStore set_delegate:]`, macOS 15+) with
  `webCryptoMasterKey:`, for page, dedicated-worker and service-worker
  IndexedDB alike. One delegate (`shell/webcrypto/delegate.rs`) is set on
  every store the app's webviews use: the default store (the main webview,
  browser tabs and the Canvas sign-in window, whose `data_directory` WKWebView
  ignores) first thing in setup, and each dock store in `configure_store`, in
  the same main-thread turn that creates its page. Both run before the run
  loop turns, and WebKit asks only when a web process does, over the run
  loop, so no page can ask first. The delegate answers the 16 raw bytes, or
  nil, which sends WebKit to its own item. A store without `set_delegate:` is
  left to WebKit, logged once.
- **keyd is asked once per process.** WebKit asks on the main thread, on
  every wrap and unwrap, and keeps nothing. The first ask runs `fetch` on a
  thread of its own and queues WebKit's completion handler; asks meanwhile
  queue theirs. The answer comes back to the main thread
  (`run_on_main_thread`), calls each handler exactly once, and is kept for
  every later ask (`cache.rs`), `None` included: a failed first read leaves
  WebKit on its keychain item until the app restarts. The log says
  `webcrypto: store <uuid|default> asks Oculus for the master key` per store,
  then `WebKit asked for its master key; asking oculus-keyd` and `answered N
  waiting request(s) with keyd's key` (or `with nothing; …`).

## A rejected Canvas request signs in once and is sent once more

`State::forward` (`app/keyd/core/src/ops/forward.rs`) judges each `canvas`
answer (`forward/rejection.rs`). The session is **rejected** when it is
missing, the answer is a 401 (except one whose body says `"status":
"unauthorized"`: Canvas telling a signed-in user "not allowed", which a new
login cannot fix), or the answer is a 301, 302, 303, 307 or 308 whose
`Location` is the SSO host or Canvas's `/login`. A 403, 404, 5xx or a redirect
to a file host is never a rejection.

- **One login per burst.** The generation is read before the session. A
  rejected request that finds it changed was sent with a session another
  request has since replaced, so it just retries; otherwise it runs
  `okta::sign_in` with `Trigger::Forward` through the same single flight as
  `ensure_signed_in` (a waiter takes the running attempt's outcome) and the
  guard. A sign-in the guard refuses is re-checked against the generation
  before it is believed, so a request that raced a finished sign-in still
  retries.
- **At most one sign-in and one retry per request.** The second answer is
  returned whatever it is. The request body is still in hand, and a streamed
  request is judged at its head, before any body byte is relayed.
- **A refused sign-in is the answer, with the reason.** The reply is the
  rejected one (its `set-cookie` stripped and not absorbed) plus a `signin`
  field in the shape of `ensure_signed_in`'s error
  (`{"result":"error","code":…,"detail"/"wait_secs"/"factors"}`); the client
  reads it as `RawResponse.signin`. With no session and no way to make one,
  the error is `missing` carrying the same field
  (`KeydError::NoSession`). No cookie is in any reply or log.
- **The socket must wait.** A rejected request goes quiet while the flow runs
  (up to a dozen 45 s Okta requests, a wait for a fresh TOTP window, another
  caller's attempt first), so a session route's client timeout is
  `client::SESSION_TIMEOUT`, ten minutes per read.

## Every sign-in attempt goes through one guard

The startup probe, the browser, a rejected Canvas request in keyd and
`oculus auth auto` each ask for a sign-in on their own, from three processes,
and Okta locks the account after too many attempts. So
`keyd_core::okta::sign_in` checks one record, `canvas-session/sign-in.json`,
before each attempt, and logs every attempt with its caller to
`okta-sign-in.log`. The guard is that one function (`okta/guard.rs`); nothing
calls the flow around it. `sign_in` takes who asked: the trigger (`manual`,
`startup`, `browser`, or keyd's own `forward`) and the caller's role (`app` or
`cli`), which keyd reads from the connection.

**The rules** (`guard::admit`):

- **Every attempt, manual included, starts at least 60 s after the last**
  (`LoginError::Waiting`).
- **Automatic attempts** (every trigger but `manual`, `forward` included)
  also wait 10 min after any attempt, then 1 h after
  two failures in a row and 6 h after three, and stop at a pause. A success
  resets the count. A network failure waits but does not count: Okta gave no
  verdict.
- **A manual attempt** (`Trigger::Manual`: Connect, `oculus auth auto`) skips
  that back-off and the pause, because a person is waiting. After three
  failed manual attempts in a row, with no success or credential save between,
  it is held to the back-off and the pause like an automatic one.
- **A lockout or a rejected password pauses automatic sign-in**, and only a
  manual attempt from the app lifts it, or saving credentials, or a person
  signing in in the login window or a browser tab. A manual request from the
  CLI is refused with `Paused` pointing at Settings → Canvas and
  `oculus auth setup`, so a command (or an agent running one) cannot keep
  retrying a locked account. A factor Okta offers that the flow cannot answer
  also pauses; any manual attempt lifts that one.
- **Saving credentials or a person's sign-in** (`resume_automatic_sign_in`)
  clears the failures, the pause and the back-off, but not the 60 s between
  attempts.
- After a sign-out no automatic attempt starts at all, and none is recorded
  (`LoginError::SignedOut`).
- An attempt costs a rejected request nothing it can retry: a refusal comes
  back as `signin` on the reply, never as a second attempt.

**The record** (`okta/guard/record.rs`) holds `last` (when the last attempt
started), `failures`, `paused`, `credentials_paused` (the pause is a lockout or
password; a record that does not say counts as one), `manual_failures` and
`forgiven` (a save cleared the back-off).

- It is replaced atomically (a temp file in the same directory, fsynced, then
  renamed over it) under a lock on a sibling file, `sign-in.json.lock`, so a
  crash leaves the old record or the new one and never an empty file. The lock
  file is created on first use, so a record with none beside it is read as it
  is.
- A file that is missing is a blank record: nothing has been attempted. A file
  that exists but is empty, truncated, mistyped or short of a field is
  *damaged*, and a blank record would forget a lockout pause, so an automatic
  attempt is refused (`LoginError::Paused`, naming the file) and the file is
  left untouched. A manual attempt, saving credentials or a person's sign-in
  writes a good record over it.
- An attempt is also refused when the record cannot be locked, read or saved:
  with no count of earlier attempts, running could lock the account. Only a
  manual attempt from the app still runs, since a person is waiting.

## Ed mints its `x-token` from Canvas

Ed has no third-party OAuth; every API call carries the web app's `x-token`
JWT. `app/src-tauri/src/sources/ed/lti.rs` mints it by walking the Canvas → Ed LTI 1.3
launch (tool page → `oidc_login` → Canvas `/api/lti/authorize` → `launch` →
one-shot `?_logintoken=` → `POST /api/login_token`). Tokens last about two
weeks, are renewed with `POST /api/renew_token` on each sync, and a dead one
is re-minted. The token lives in keyd's vault (`session.ed`) and is attached by
keyd's `ed` route; `ed.rs` never reads it back, except that a renewal's body
carries the new token, which it hands to `session_put`. The Canvas legs of the
walk go through the `canvas` route, the other hosts' cookies are jarred in the
process, and `POST /api/login_token`, which has no session to attach, is a
direct request. `oculus auth ed <TOKEN>` is a manual override: the token is
checked against `/api/user` directly, and only a good one is stored.

## Echo360 has no stored credential

Each session is minted on demand from the Canvas session (the tool page comes
through keyd's `canvas` route) by POSTing the course's LTI external-tool form (see
[sync.md](./sync.md#echo360-is-an-lti-launch-with-up-to-two-streams)), and
`app/src-tauri/src/lectures/mod.rs` caches it per course.

## An embedded Echo360 video launches per link and plays on a Bearer token

A video embedded in a Canvas page is a different LTI launch from the course
one, keyed by the embed's `linkId` (`app/src-tauri/src/sources/echo360/embed/`):

1. GET the embed's Canvas `external_tools/retrieve` URL through keyd's
   `canvas` route. It answers an LTI form; a page without one is reported as a
   possibly lapsed Canvas session. The URL comes from page markdown, so
   `embed::retrieve_url` accepts only a retrieve on the profile's Canvas of a
   `<region>/lti/…?linkId=` URL naming that same link, and rebuilds it. The
   regions are Echo360's own hosts (`echo360.org`, `echo360.net.au`, …,
   listed in `app/src-tauri/src/sources/echo360/embed/region.rs`), since the form Canvas signs is posted there.
2. POST the form to Echo360 with redirects off; its target must be a region
   too. It answers a redirect to
   `/ui/player/<mediaId>?sessionId=…&secureLinkAccessDataId=…` on that region,
   which every later call uses; the only cookie is `PLAY_SESSION`, and there
   is no `ECHO_JWT`.
3. GET `/api/ui/sessions/<sessionId>` on the same cookie jar, at once: the
   session id goes stale within minutes. It answers 204 with the JWT in the
   `token` *response header*.
4. Send that JWT as `Authorization: Bearer` to
   `/api/ui/echoplayer/secure-link-access-datas/<sl>/media/<mediaId>/player-properties`.

One Bearer serves every link on its region and lives about 13 hours, so the
app keeps one in `EmbedBearer`, with the region that minted it, and reuses it
for that region's links until five minutes before its `exp`. A link's media
and secure-link ids never change, so they are cached in `echo_embeds`
([sync.md](./sync.md#an-embedded-echo360-video-is-resolved-and-downloaded-by-its-card));
with both cached and a live Bearer, no launch runs, which matters because every
launch records a view in Echo360. A Bearer turned down with 401/403 is dropped
and replaced from a fresh launch, once.

The CDN takes neither the Bearer nor a cookie. A URL is signed by **replacing**
its query with the `queryString` of the `sourceQueryStrings` entry whose
`uriPattern` (a `*` glob) matches it (`embed::sign`); appending to the
existing query 403s, as does the bare URL.

## Gotchas

- Never POST Echo360's `last-played-to-seconds`: an embed launch already records a view under the student's name, and Oculus reports no playback.
- Every sign-in by a person must end in `auth::session_established` (`session_mark(true)`), and keyd's own sign-ins mark themselves — the startup probe reads the flag first, and without it a valid session opens disconnected.
- keyd calls (`session_get`, `session_status`, `session_put`) can wait on the keychain prompt for 60 s: never from the main thread or a WebKit callback.
- Holding a `canvas_session` is not success: Canvas issues an anonymous one before auth, so `complete_saml` accepts one only after posting an assertion and `sign_in` verifies it against `/api/v1/users/self` before writing it.
- Answer `currentAuthenticator`'s challenge before reading the chooser — OIE offers `select-authenticator-authenticate` beside every challenge, and taking it loops forever.
- The login page names `stateToken` several times; the first is a fragment introspect rejects as "session has expired", so `state_token_candidates` tries every plausible one.
- Only `/idp/idx/introspect` takes `stateToken`; every later call sends `stateHandle`.
- An Okta credential call falls back to the keychain only on `KeydError::Absent`, and a sign-in never falls back: with keyd absent it is `LoginError::Broker`, and nothing mints a session outside keyd.
- Never run the flow around the guard or loop a sign-in on a 401: a burst of rejected requests must be one login, or Okta locks the account.
- A rejected password is cleared and automatic sign-in paused, or every automatic path would replay it until Okta locks the account.
- A TOTP seed cannot be recovered from codes; getting it means re-enrolling the factor.
- `setCookies:completionHandler:` must get a real block — nil segfaults the app seconds later from a WebKit-only stack.
- WebKit drops an API-set cookie that would replace an HttpOnly one a server set, with no error. Seeding deletes same-named cookies for the host and its parent domains before writing host copies, and records the copies so capture can prefer a server's rotation.
- `/login/session_token` answers 403 to a cookie session; it wants an access token, which is disabled here.
- The data store's delegate setter is `set_delegate:`; `_setDelegate:` does not exist and throws.
- The navigation delegate's `_webCryptoMasterKeyForWebView:` misses service workers, which read WebKit's keychain item (and create it if absent); only the data store's delegate covers every IndexedDB.
- A wrong WebCrypto master key fails silently: IndexedDB `get` resolves `undefined`, the records stay, and WhatsApp and Messenger sign out. Answer keyd's key or nil, nothing else.
- WebKit holds the data store's delegate weakly, so the delegate lives for the process; and it checks each `webCryptoMasterKey:` handler is called exactly once.
- Ed's LTI redirect walk jars cookies per host, and Canvas's legs go through keyd, which attaches the cookie only to Canvas; it must never reach edstem.org.
- UniMelb's `/login` is a portal page, not its IdP — detection tries `/login/saml` after it, and UniMelb's profile keeps `/login/saml` as its login path.
- The keychain service `com.oculus.unimelb-sso` is an identifier: renaming it orphans every saved password and setup key.

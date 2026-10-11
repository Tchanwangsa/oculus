# Music

Spotify played natively: Oculus is a Spotify Connect device ("Oculus")
through librespot, with audio out of CoreAudio and no Spotify app running.
Premium only. Nothing connects at startup; a session starts only from
`music_connect` or a finished `music_login`.

## Where

| Piece | Location |
| --- | --- |
| The module and its rules | `app/src-tauri/src/music/mod.rs` |
| The session task: player, connect, reconnect, stop | `app/src-tauri/src/music/session.rs` |
| The snapshot, the `music-state` event, the Spirc, run and login handles | `app/src-tauri/src/music/state.rs` |
| Player events folded into the snapshot | `app/src-tauri/src/music/events.rs` |
| A track and its cover URL | `app/src-tauri/src/music/track.rs` |
| The OAuth login and its callback on port 8898 | `app/src-tauri/src/music/login.rs` |
| The stored credentials in keyd; keyd's `get` and its allowlist | `app/src-tauri/src/music/credentials.rs`, `app/keyd/core/src/ops/get.rs`, `app/keyd/core/src/names.rs` |
| librespot's log lines, and the ones that mean a track will not play | `app/src-tauri/src/music/logs.rs` |
| The commands | `app/src-tauri/src/music/commands.rs` |
| The frontend's types and `invoke` wrappers | `app/src/lib/music/index.ts` |

## librespot is pinned to a dev-branch commit

`librespot-core`, `-connect`, `-metadata`, `-playback` (default features:
the rodio backend over CoreAudio, native TLS) and `-protocol` are git
dependencies at one rev of librespot's dev branch, not crates.io 0.8.0:
Spotify keeps closing the private endpoints librespot uses, and the fixes land
on dev long before a release. The rev is bumped by hand when playback breaks;
`cargo update` never touches it, and the lockfile keeps tauri at 2.11.2 and
vergen at the versions librespot's build script accepts.

librespot's `Cache` is never constructed: no `credentials.json`, no audio file
cache, no volume file. While a track streams, librespot buffers it in a
temporary file in the OS temp dir that is deleted when the track is dropped.

## The login is librespot's client id, in the browser, back on port 8898

- **OAuth PKCE with librespot's own client id** (`SessionConfig::default()`)
  and its scope list, redirecting to `http://127.0.0.1:8898/login`, the one
  address that id is registered for. A Client ID of our own would cap
  Spotify at five users and need a Premium developer account behind it.
- **`login.rs` drives the `oauth2` flow itself**, because librespot-oauth
  opens the browser on its own and never hands the URL out. The port is bound
  first, so a busy port fails at once with "Port 8898 is in use" in the
  snapshot and no browser opens; the URL then goes through the opener plugin.
- **A waiting login can be replaced or cancelled.** A second `music_login`
  cancels the first and opens the page again; `music_disconnect`, sign-out
  and app exit cancel it. Only the current login writes to the snapshot.
- **The token exchange is `reqwest::blocking`**, which panics on a thread
  inside a tokio runtime, so the login runs on a plain `std::thread`, never
  `spawn_blocking`.
- The access token is used once: the first connect trades it for reusable
  credentials, which every later connect uses.

## The credentials live in keyd, read back by an app-only `get`

- **After each connect, `music.spotify` holds a small JSON string**: the
  canonical username, librespot's stored-credentials blob
  (`Session::auth_data`, base64) and the Connect device id. The device id is
  made at login and kept, so the device keeps one identity across launches.
  It is written only when it differs from what was loaded.
- **keyd's `get` is the one op that returns a stored value**, for the names in
  `names::READABLE` (only `music.spotify`) and to the app alone: librespot logs
  in with the blob itself, so there is no request keyd could attach it to the
  way `forward` attaches a key ([auth.md](./auth.md#spotifys-credentials-are-the-one-value-keyd-reads-back)).
- **keyd calls run off the async runtime** (`runtime::blocking::run`); the
  save after a connect runs in its own task, so the Spirc keeps being polled
  while keyd waits on a keychain prompt. A failed save leaves playback running
  and says "Signed in, but the login was not kept".
- **keyd absent is an error, not a fallback**: the snapshot says so and
  nothing is written to the keychain.
- `music_sign_out` deletes the item; `music_disconnect` keeps it. Canvas
  sign-out (`sign_out`, `session_clear`) never touches it.

## One session task reconnects with backoff and stops on an auth error

1. `session::start` registers a run, or does nothing while one is running.
   Each run has an id, and the snapshot ignores writes from a run that is no
   longer current, so a stopped task cannot change what the user sees.
2. The task makes one player (normalisation off, rodio sink) and mixer, then
   connects as "Oculus", device type computer, at 50% volume.
3. Every attempt gets a new `Session`, handed to the player with
   `set_session`: a Session cannot be reused once its Spirc has ended.
4. A connection that drops without a stop is rebuilt after a backoff of 2 s
   doubling to 60 s, reset once a connection has lived a minute; the snapshot
   says it is reconnecting.
5. `PermissionDenied` or `Unauthenticated` ends the task with `needs_login`
   and the reason in `error`. It is never retried.
6. A stop (disconnect, sign-out, a new login, app exit) sends the Spirc its
   shutdown, which pauses and leaves Spotify's device list, gives it 10 s,
   then shuts the session down.

`music_play_context` activates the device before it loads: Spotify ignores a
load or transfer sent to a device that is not active.

## Rust owns the state, and `music-state` carries all of it

`state::Snapshot` is sent whole to the main webview on every change that
alters it; `music_state` is the mount-time read. Fields, snake_case:
`connected`, `connecting` (a login, connect or reconnect is under way),
`needs_login`, `account`, `device_name`, `track` (`uri`, `name`, `artists`,
`album`, `cover_url`, `duration_ms`) or null, `position_ms` with
`position_at_ms`, `playing`, `volume` (0–100), `shuffle`, `repeat` and
`error`.

- **The position is stamped, not ticked.** `position_at_ms` is the epoch ms at
  which `position_ms` was true; while playing, the UI adds the time since
  (`positionNow` in `app/src/lib/music/index.ts`). Rust sends no per-second
  events.
- **`error` is the latest failure** (keyd, connect, auth, a track that cannot
  play) and stays until the next connect or `music_play_context`.

## A track that cannot play is often only a log line

librespot reports a missing audio key as a warning ("Unable to load key,
continuing without decryption") and goes on reporting `Playing` over silence.
`logs.rs` installs the app's only `log` logger when the first session starts:
it prints librespot's warnings and errors to stderr and turns the ones that
mean a track will not be heard (audio key, decrypt, load, availability,
Login5's `INVALID_CREDENTIALS`) into the snapshot's `error`. The player's
`Unavailable` event does the same.

## Covers come from Spotify's image CDN

`cover_url` is `https://i.scdn.co/image/<hex file id>` for the album's largest
cover at most 640 px wide, from the `AudioItem` that `TrackChanged` carries.
librespot builds the URL from an account attribute that may name another
host, so only the file id is kept from it. `tauri.conf.json` sets no CSP; one
added later needs `https://i.scdn.co` in `img-src`.

## Commands

| Command | What it does |
| --- | --- |
| `music_state` | The snapshot |
| `music_login` | Stops any session, opens the login, connects once it is approved |
| `music_connect` | Connects from keyd's credentials; `needs_login` when there are none |
| `music_disconnect` | Stops the session and any waiting login; keeps the credentials |
| `music_sign_out` | Disconnects and deletes `music.spotify` from keyd |
| `music_play_pause`, `music_next`, `music_prev` | Playback on the connected device |
| `music_seek(position_ms)`, `music_volume(percent)` | Seek; volume 0–100 |
| `music_play_context(uri)` | Activates the device, loads a `spotify:` context and plays it |

Playback commands fail with "Not connected to Spotify" without a session.

## Gotchas

- `reqwest::blocking` inside a tokio context panics — the login stays on a plain thread.
- `load` and `transfer` are ignored by an inactive device — activate first.
- A Session whose Spirc ended cannot connect again — make a new one and `set_session`.
- `Playing` is not proof of audio — a missing audio key plays silence; watch `error`.
- Constructing librespot's `Cache` writes the account's credentials to disk in plain text — never use it.
- `cargo update` on librespot moves it off the pinned rev and can move tauri off 2.11.2 — bump the rev by hand.

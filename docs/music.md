# Music

Spotify played natively: Oculus is a Spotify Connect device ("Oculus")
through librespot, with audio out of CoreAudio and no Spotify app running.
The same session lists the account's playlists, searches and lists its
Connect devices ([music-library.md](./music-library.md)).
Premium only, and off until Settings → Music turns it on. A session starts
from `music_connect`, a finished `music_login`, or at startup while that
switch is on.

## Where

| Piece | Location |
| --- | --- |
| The module and its rules | `app/src-tauri/src/music/mod.rs` |
| The session task: player, connect, reconnect, stop | `app/src-tauri/src/music/session.rs` |
| The snapshot, the `music-state` event, the Spirc, run and login handles | `app/src-tauri/src/music/state.rs` |
| Player events folded into the snapshot | `app/src-tauri/src/music/events.rs` |
| Where a command goes: here, the device playing elsewhere, a pull, an activate or nothing | `app/src-tauri/src/music/control.rs` |
| Remote control of another device: its commands' requests, its playback for the snapshot | `app/src-tauri/src/music/remote/` |
| Connect devices and transfer | `app/src-tauri/src/music/devices.rs` |
| The output on this Mac: the choice, cpal's devices, the player's sink | `app/src-tauri/src/music/audio/` |
| A track and its cover URL | `app/src-tauri/src/music/track.rs` |
| The library: rate-limit gate, playlists and their tracks, covers, search | `app/src-tauri/src/music/library/` |
| The duck a focus mode calls | `app/src-tauri/src/music/duck.rs` |
| The OAuth login and its callback on port 8898 | `app/src-tauri/src/music/login.rs` |
| The stored credentials in keyd; keyd's `get` and its allowlist | `app/src-tauri/src/music/credentials.rs`, `app/keyd/core/src/ops/get.rs`, `app/keyd/core/src/names.rs` |
| librespot's log lines, and the ones that mean a track will not play | `app/src-tauri/src/music/logs.rs` |
| The commands | `app/src-tauri/src/music/commands.rs` |
| Startup's connect, from the `music` settings row | `app/src-tauri/src/music/startup.rs` |
| The frontend's types and `invoke` wrappers | `app/src/lib/music/index.ts` |
| The snapshot mirror, the switch, the app-level listener | `app/src/stores/music/musicStore.ts`, `app/src/stores/music/musicPrefsStore.ts`, `app/src/hooks/music/useMusicState.ts` |
| Music against lecture audio | `app/src/lib/music/exclusive/rule.ts`, `app/src/lib/music/exclusive/index.ts`, `app/src/lib/lectures/media/library.ts` |
| Settings → Music | `app/src/pages/settings/integrations/MusicPage.tsx`, `app/src/components/settings/music/` |
| The rail tile and its popover | `app/src/components/sidebar/MusicTile.tsx`, `app/src/components/music/NowPlaying.tsx` |
| The full player (`/music`) and its parts | `app/src/pages/tools/MusicPage.tsx`, `app/src/components/music/` |
| Up next, playlists and their tracks, and search under the player | `app/src/components/music/library/` |
| The device menu (Spotify devices, output on this Mac) | `app/src/components/music/devices/`, `app/src/components/music/VolumeRow.tsx`, `app/src/components/music/PlayingElsewhere.tsx` |

## Settings → Music is the one switch, and the only startup connect

- **The `music` settings row is `{"enabled": boolean, "output": string |
  null}`**, absent (off, system default) by default. `musicPrefsStore`
  writes it, merging into the stored object so keys it does not know
  survive; on saves and calls `music_connect`, off saves and calls
  `music_disconnect`, which keeps the stored login. The output is written by
  the device menu ([below](#the-output-on-this-mac-moves-mid-track-without-rebuilding-the-player)).
- **At startup Rust reads the row** (`setting_blocking`) on the thread that
  first installs keyd, since the stored login is keyd's to hand back: it
  applies `output` whatever the switch says, and starts a session when it is
  on. Anything but `"enabled": true` connects nothing.
- **The page draws the session while the switch is on**: a status dot and
  line (connected as the account, connecting, not signed in, could not
  connect), "Log in with Spotify" when `needs_login`, else Connect or
  Disconnect, and Sign out, which deletes the stored login. The device name is
  read-only, and the latest error sits under it.
- **Nothing on the page plays.** Its buttons are logins and connections; a
  "test playback" button would be a side-effect call from Settings
  ([harness.md](./harness.md#no-model-is-ever-probed)).

## The player is a rail tile and a side panel item

- **`useMusicState` (mounted once in `AppLayout`) mirrors the snapshot** into
  `musicStore`: one `music_state` read on mount, then `music-state`. It also
  loads the switch and wires the exclusive-playback rule.
- **The rail tile sits in the group after the sections**, under its own
  rule, only while the switch is on and a session is up or a track is
  loaded; otherwise it is not drawn. It shows the cover (a note glyph without
  one) with the rail's corner marks: a spinner while connecting, a brand dot
  while playing. A click opens a popover below and to its right: the track,
  seek, previous / play-pause / next, volume with the device button, and
  "Open player".
- **The device button** (`DeviceMenu`, ending the volume row here and on
  `/music`) opens "Spotify devices" (active checked, a click transfers) and
  "Output on this Mac" ("System default (<device>)", then each output). Both
  lists are read on open, never on a timer; errors sit inline. While another device plays, the button is brand-tinted, "Playing on
  <device>" shows under the volume with a "Play here" button
  (`music_play_here`) that moves playback to Oculus, and the transport
  controls that device ([below](#commands-control-the-device-playing-wherever-it-is)).
- **The player shows what the device playing plays**: `shownPlayback` in
  `app/src/lib/music/index.ts` picks the snapshot's `remote` while another
  device plays and Oculus's own fields otherwise, and the cover, title, seek,
  play/pause, shuffle, repeat, volume, the rail tile, the playing row in the
  library and `useMusicPosition` all read it. Another device's volume is sent
  on release, not as it moves, since each change is a request to Spotify; a
  device that takes no volume disables the slider.
- **"Open player" opens `/music` as the page** in the focused pane of the
  tab in front (`navigateActive`, as the rail's sections do); its
  `recentKey` is `music`, so there is one player item however it is opened.
  The page adds the album, elapsed and total, shuffle and repeat, a queue
  button for what plays next, the account's playlists with their tracks and
  a search, in two columns when the page is wide
  ([music-library.md](./music-library.md)), and,
  with no session or track, a "Not connected" view linking to Settings → Music.
- **The clock ticks in the UI**: `useMusicPosition` re-renders on each whole
  second of `positionNow` while playing, the pane is on screen and the
  window is visible. A slider keeps its dragged value until the next
  snapshot confirms it (`useSliderDraft`); seek is sent on release, this
  device's volume as it moves.
- **Play and pause are separate commands** from the toggle, so a click on a
  stale button, or the rule below, never does the opposite. The player's
  pause is `music_pause_player`, which pauses what it shows; `music_pause` is
  the rule's, which pauses this Mac only.

## Music and lecture audio never play together

The one started last wins, and the other pauses. Nothing resumes by itself,
on either side.

- **Music starting on this Mac** (`playing` going false to true in the
  snapshot, whether from Oculus's player or from the Spotify app on a phone
  that chose Oculus) pauses the lecture (`pauseLecturePlayback`) and every
  library video.
- **A lecture or library video starting** pauses music with `music_pause`,
  which pauses Oculus only: a lecture on the Mac never pauses a phone. The
  rule reads the snapshot's own `playing`, never `remote`, so music playing
  on another device is not music here.
  The lecture's start is the playback owner's `playing`; a library video's is
  its element's `play`, which `lib/lectures/media/library.ts` reports.
- **Both starting in one step keeps the app's own sound**, the one clicked
  here. The decision is `toSilence` in `app/src/lib/music/exclusive/rule.ts`,
  a pure function of the previous and next state.

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
   connects as "Oculus", device type computer, at the snapshot's volume
   (100% until changed). librespot's soft volume is a 60 dB log curve, so 50%
   is about −30 dB; at 100% the Mac's volume sets the loudness. librespot
   restores no volume (no `Cache`), so each reconnect passes the user's.
3. Every attempt gets a new `Session`, handed to the player with
   `set_session`: a Session cannot be reused once its Spirc has ended. Each
   session subscribes to the Connect cluster before its Spirc starts
   ([music-library.md](./music-library.md#the-device-list-is-spotifys-pushed-cluster)).
4. A connection that drops without a stop is rebuilt after a backoff of 2 s
   doubling to 60 s, reset once a connection has lived a minute; the snapshot
   says it is reconnecting.
5. `PermissionDenied` or `Unauthenticated` ends the task with `needs_login`
   and the reason in `error`. It is never retried.
6. A stop (disconnect, sign-out, a new login, app exit) sends the Spirc its
   shutdown, which pauses and leaves Spotify's device list, gives it 10 s,
   then shuts the session down.

## Commands control the device playing, wherever it is

After a phone or the desktop app takes playback, librespot's Spirc drops
every command but `activate` and `transfer` ("will be ignored while Not
Active"). So `control.rs` first finds where playback is — here (the
snapshot's `active`), on another device (the last cluster push names an
active device that is not this one), or nowhere — and `control::step`, a
pure function, picks what the command does:

| Command | Here | Another device plays | Nothing plays |
| --- | --- | --- | --- |
| `music_play`, `music_play_pause`, `music_next`, `music_prev`, `music_seek`, `music_shuffle`, `music_repeat`, `music_volume` | Spirc | Remote | Pull, then Spirc |
| `music_pause_player` (the player's pause) | Spirc | Remote | Nothing |
| `music_play_context`, `music_play_track` | Spirc | Remote `play` | Activate, then Spirc |
| `music_add_to_queue` | Spirc | Remote `add_to_queue` | Refused |
| `music_play_here`, `music_transfer` to Oculus | Spirc `play` | Pull, then play | Pull, then play |
| `music_pause` (the exclusive rule), `music_duck` | Spirc | Nothing | Nothing |

- **Remote** sends the command to the device playing (`remote/`), from this
  device's Connect id to the active one's, through the library's spclient
  sender with the session's Login5 and client tokens. Player commands are
  Spotify's connect-state player command API, the request Spotify's own apps
  send to remote-control each other: `POST
  /connect-state/v1/player/command/from/<own>/to/<active>` with
  `{"command": {"endpoint": …, …, "logging_params": {}}}`. The body's shape
  follows librespot's receiver (`core/src/dealer/protocol/request.rs` at the
  pinned rev), which needs `logging_params` on every command: `pause`,
  `resume`, `skip_next`, `skip_prev`; `seek_to` with `value` and `position`;
  `set_shuffling_context`, `set_repeating_context` and `set_repeating_track`
  with `value`; `add_to_queue` with a `queue`-provider track; `play` with
  the context's `uri` and `url`, a `play_origin`, and `options.skip_to` for a
  track inside it (a track with no context is sent as its own context).
  Volume is `PUT /connect-state/v1/connect/volume/from/<own>/to/<active>`
  with `{"volume": 0–65535}`. The play/pause toggle sends `pause` or `resume`
  by the other device's playing state; repeat off clears both repeats, as
  here. A command is sent once: a non-2xx fails it with "Spotify refused the
  command on <device> (<status>)" and is logged with the start of Spotify's
  answer, and nothing falls back to pulling playback here. The outcome
  arrives with the next cluster push.
- **Pull**: `Spirc::transfer`, which asks Spotify to move the current
  playback here. It lands later as a dealer request, so a command sent at
  once would still be dropped; the command waits up to 5 s for `active`,
  then is sent. If the transfer never lands, `activate` takes over locally
  and the command follows it. Play/pause pulls and plays: a toggle would
  pause what the pull brings over.
- **Activate** (a load with nothing active): `activate` alone. The Spirc
  handles commands in order and `activate` makes it active at once, so the
  load after it is kept.
- **Refused**: with nothing playing anywhere there is no queue, so
  `music_add_to_queue` fails with "Play something first, then add to the
  queue".
- **Nothing**: `music_pause` and `music_duck` never touch another device;
  a pause never pulls playback here.

`active` follows the player's `SessionConnected` (the Spirc activated, from a
command, a transfer or a play from the phone) and `SessionDisconnected`
(another device took over), which the Spirc emits while the session itself
stays up.

### The snapshot's `remote` is the other device's playback

While another device is active, each cluster push sets `remote` from its
player state (`remote/playback.rs`): the device's name, the track,
`position_as_of_timestamp` at `timestamp` as `position_ms` and
`position_at_ms`, `playing` (playing and not paused), shuffle, repeat
(context or track), and the device's volume from its device info (none when
its capabilities disable volume). A push without a player state keeps what
was known of the same device; Oculus becoming active, nothing being active,
or the connection dropping clears it. The snapshot's own `track`,
`position_ms`, `playing`, `volume`, `shuffle` and `repeat` stay Oculus's,
which the duck and the exclusive rule rely on.

The track is named first from the metadata Spotify's apps attach to the
track they play (`title`, `album_title`, `artist_name`, `image_large_url` or
`image_url`) when it is there, then from the library's row for its URI
(`music_tracks`' cache): a row already kept fills it at once; otherwise the
row is fetched once per URI and the snapshot goes out again when it comes.

## The output on this Mac moves mid-track without rebuilding the player

"System default" (the default) or a CoreAudio output by name, from the cpal
and rodio versions librespot's rodio backend uses. `music_set_output` sets
`audio::Choice` and the snapshot's `output`.

- **The player's sink is our own** (`audio/sink.rs`), librespot's rodio
  sink rebuilt: librespot builds its sink once, inside `Player::new`, with no
  way to reopen it, and a new player means a new Spirc and Session. On every
  start, stop and write the sink checks the choice and reopens itself on a
  new one, so the switch lands within a packet and the queue and position
  stay.
- **"System default" follows the Mac by itself**: cpal opens the default
  device as CoreAudio's default-output unit, which moves with the Mac's
  default output (AirPods connecting, Control Center), so nothing listens or
  polls for it.
- **A chosen device that disappears** (cpal reports it gone) falls back to
  the default with "<name> is not available; playing on the system default"
  in `error`; while it is missing the sink looks for it every 2 s as it
  plays, moves back when it returns, and takes its message back.
- **No output at all** fails `start` or `write`, which librespot answers by
  pausing, with the reason in `error`. `stop` never fails (librespot exits the
  process on that) and waits at most 1 s for queued audio: a stream whose
  device has gone stops pulling, and librespot's own sink then waits forever.

## `music_duck` is the seam a focus mode calls

A focus mode lowers music with `music.duck(level)` and puts it back with
`music.unduck()` (`music_duck` with a level or null). No mode logic lives in
the music module.

- **A duck sets the volume to `level`% of the volume before the first duck**,
  which is kept; a second duck is a share of that kept volume, not of the
  ducked one, and does not replace it.
- **The unduck restores the kept volume**, and does nothing without one.
- **A volume the user sets while ducked wins**: `music_volume`, or a
  `VolumeChanged` from the Spotify app that is not the duck's own echo
  (within 1%), drops the kept volume, so the unduck leaves their choice.
- A dropped connection or a stop forgets the duck, and so does a duck sent
  while another device plays: it does nothing.

## Rust owns the state, and `music-state` carries all of it

`state::Snapshot` is sent whole to the main webview on every change that
alters it; `music_state` is the mount-time read. Fields, snake_case:
`connected`, `connecting` (a login, connect or reconnect is under way),
`needs_login`, `account`, `device_name`, `active` (Oculus is the active
Connect device), `playing_on` (the active device's name when it is another,
from the cluster), `output` (the chosen output, null for the system
default), `remote` (the other device's playback, [above](#the-snapshots-remote-is-the-other-devices-playback)), `track` (`uri`, `name`, `artists`,
`album`, `cover_url`, `duration_ms`) or null, `position_ms` with
`position_at_ms`, `playing`, `volume` (0–100), `shuffle`, `repeat`,
`up_next` (`uri`, `queued`; [music-library.md](./music-library.md#up-next-comes-from-the-spirc-while-oculus-plays-and-the-cluster-otherwise))
and `error`.

- **The position is stamped, not ticked.** `position_at_ms` is the epoch ms at
  which `position_ms` was true; while playing, the UI adds the time since
  (`positionNow` in `app/src/lib/music/index.ts`). Rust sends no per-second
  events.
- **`error` is the latest failure** (keyd, connect, auth, a track that cannot
  play, an output missing) and stays until the next connect or load; the
  sink takes back only its own message.

## A track that cannot play is often only a log line

librespot reports a missing audio key as a warning ("Unable to load key,
continuing without decryption") and goes on reporting `Playing` over silence.
`logs.rs` installs the app's only `log` logger when the first session starts:
it prints librespot's warnings and errors to stderr and turns the ones that
mean a track will not be heard (audio key, decrypt, load, availability,
Login5's `INVALID_CREDENTIALS`) into the snapshot's `error`. The player's
`Unavailable` event does the same.

Every line the music module prints goes through `elog!` (`music/mod.rs`),
which drops the line rather than panicking when stderr is a closed pipe, as
`eprintln!` does.

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
| `music_play_pause`, `music_play`, `music_pause_player`, `music_next`, `music_prev` | Playback of the device playing, here or another ([routing](#commands-control-the-device-playing-wherever-it-is)) |
| `music_pause` | Pauses Oculus only; never starts, pulls or touches another device (the exclusive rule's) |
| `music_shuffle(on)`, `music_repeat(on)` | Shuffle; repeat the context (off also clears a repeated track) |
| `music_seek(position_ms)`, `music_volume(percent)` | Seek; volume 0–100, which here drops a held duck |
| `music_play_context(uri)` | Plays a `spotify:` context on the device playing elsewhere, else loads it here |
| `music_play_track(uri, context_uri?)` | Plays a track from its place in the context (its album, or the playlist it was picked in), or alone without one; on the device playing elsewhere, else here |
| `music_add_to_queue(uri)` | Adds a track to the queue of the device playing; refused while nothing plays |
| `music_play_here()` | Moves playback here and plays |
| `music_duck(level?)` | A level ducks to that share of the kept volume; null restores it; nothing while inactive |
| `music_playlists(refresh)`, `music_search(query)`, `music_search_more(query, kind, offset)` | The playlists, kept until `refresh`; up to ten each of tracks, artists, albums and playlists; a page of 20 of one kind and the next offset ([music-library.md](./music-library.md)) |
| `music_playlist_covers(uris)`, `music_playlist_tracks(uri)`, `music_album_tracks(uri)`, `music_tracks(uris)` | Covers made for playlists without one; a playlist's or album's track URIs; track rows by URI ([music-library.md](./music-library.md)) |
| `music_devices()`, `music_transfer(device_id)` | Connect devices from the last push; move playback and play ([music-library.md](./music-library.md#the-device-list-is-spotifys-pushed-cluster)) |
| `music_outputs()`, `music_set_output(name?)` | This Mac's outputs, the default marked; choose one, null for the system default |

Playback and library commands fail with "Not connected to Spotify" without a
session.

## Gotchas

- `reqwest::blocking` inside a tokio context panics — the login stays on a plain thread.
- Every Spirc command but `activate` and `transfer` is dropped by an inactive device — go through `control::send` or `control::route`.
- A command sent right after `transfer` is still dropped — wait for `active` (`control.rs`).
- A Session whose Spirc ended cannot connect again — make a new one and `set_session`.
- `Playing` is not proof of audio — a missing audio key plays silence; watch `error`.
- Constructing librespot's `Cache` writes the account's credentials to disk in plain text — never use it.
- `music_play_pause` from the exclusive-playback rule would start music it meant to stop, and `music_pause_player` would pause a phone — the rule uses `music_pause`.
- `cargo update` on librespot moves it off the pinned rev and can move tauri off 2.11.2 — bump the rev by hand.
- A track URI is not a context librespot can load — `music_play_track` uses its album or playlist, or `from_tracks`.
- An inactive Spirc drops `add_to_queue` without a word — `music_add_to_queue` sends it to the device playing, or refuses with nothing playing (`Need::Queue`).
- librespot's receiver rejects a remote command without `logging_params` — every body carries `"logging_params": {}` (`remote/command.rs`).
- The snapshot's `track`, `playing` and `volume` are Oculus's even while a phone plays — the player reads `shownPlayback`, the exclusive rule and the duck read the snapshot's own.
- librespot's rodio sink `unwrap`s a named device that is gone and hangs on a dead stream — the sink in `audio/` replaces it.
- `.gitignore` ignores every `output/` directory — the output module is `audio/`.
- `eprintln!` panics on a closed stderr — use `elog!` in the music module.

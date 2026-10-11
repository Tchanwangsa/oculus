# Music: the Web API and Connect devices

What Spotify Connect playback alone cannot do: list the account's playlists,
search, and see and move between the account's devices. Playback itself is
[music.md](./music.md).

## Where

| Piece | Location |
| --- | --- |
| The Web API: Login5 token, rate-limit gate, requests, playlists, search, devices | `app/src-tauri/src/music/web/` |
| The Connect cluster, the device list and transfer | `app/src-tauri/src/music/devices.rs` |
| Playlists and search under the player | `app/src/components/music/library/` |
| The device menu | `app/src/components/music/devices/` |

## Under the player, the account's playlists and a search

- **"Your playlists" loads once per mount**, the first time the session is
  connected; Rust answers from memory after its first fetch, so reopening
  the player costs no call. The refresh button beside the heading asks
  Spotify again. Rows show the cover, the name, and owner · N tracks; a
  click plays the playlist (`music_play_context`).
- **Search runs on Enter only**, never per keystroke: each search is a call
  against the rate limit. Its results replace the playlists, grouped
  Playlists, Albums, Tracks, until the field is cleared (its ×, or Escape).
  A track plays from its place in its album (`music_play_track` with the
  album as context), so playback goes on through the album.
- Load and search errors sit inline where the list would be.

## The Web API runs on the session's Login5 token, with no Client ID of ours

- **The token is `session.login5().auth_token()`** for the live session:
  Login5 trades the session's stored credentials for an access token on the
  same client id the player uses. A Client ID of our own would cap Spotify
  at five users ([the
  login](./music.md#the-login-is-librespots-client-id-in-the-browser-back-on-port-8898)),
  and keymaster's `token_provider().get_token` answers 403.
- **It lives in memory only** (`app/src-tauri/src/music/web/token.rs`): kept
  for the session it came from (each connection bumps a generation in
  `app/src-tauri/src/music/state.rs`) and asked for again a minute before it
  expires. It is never written to keyd or disk and never reaches the
  frontend; Rust makes every call.
- **A 401 gets one new token and one more try**; if Login5 hands back the
  token just refused, or the retry is refused too, the call fails with
  "Spotify refused the Web API token (401)". A 403 is "Spotify refused the
  request (403)". Nothing else is retried.
- **Calls (GETs, and a PUT with a JSON body) are blocking `ureq`** through
  `runtime::blocking::run`: librespot's own HTTP client turns every non-2xx
  into a generic error and drops `Retry-After`. A `next` link that leaves
  `https://api.spotify.com/v1/` is refused.

| Endpoint | Used for |
| --- | --- |
| `GET /v1/me/playlists?limit=50` | The playlists, following `next` up to 500, kept per account until a refresh |
| `GET /v1/search?type=playlist,album,track&limit=10` | Search, ten of each |
| `GET /v1/me/player/devices` | The device list before the first cluster push, and on refresh |
| `PUT /v1/me/player` | A transfer to a device the cluster does not list |

Covers are the smallest image at least 120 px wide, as Spotify gives the URL
(a playlist's four-cover grid is on `mosaic.scdn.co`, with no file id to
rebuild an `i.scdn.co` URL from). Entries that are null or lack a URI or
name are dropped; a playlist's count is read from `tracks.total` or
`items.total`.

## A 429 closes every Web API call until its Retry-After

librespot's client id is shared by every librespot-based player and is
throttled hard. A 429 closes a gate in memory
(`app/src-tauri/src/music/web/gate.rs`) for its `Retry-After` seconds (30
when it names none, a later deadline is never shortened). Until then each
call fails at once with "Spotify is rate-limiting Oculus; try again in N s",
without touching the network: a call made early only extends the wait.
Nothing retries on its own.

## The device list is Spotify's pushed cluster

- **Spotify pushes the Connect cluster** (every device, its name, type and
  volume, and the active device's id) over librespot's dealer at
  `hm://connect-state/v1/cluster`, which the Spirc also hears. Each session
  subscribes before its Spirc starts the dealer, and the last push is kept
  until the connection drops. It is free and never rate-limited, and it
  gives the snapshot's `playing_on`.
- **Spotify pushes only on a change**, so `music_devices` answers from the
  last push when there is one, and from the Web API's `GET
  /v1/me/player/devices` before the first push and on the menu's refresh.
  Offline devices are left out; Oculus is listed first (added when a list
  lacks it), then the active device, then by name.
- **This device is the entry with the session's device id**, or, in a Web
  API list that names it differently, the first computer called "Oculus"; it
  always carries the session's device id, so a transfer to it is recognised.
- **A transfer to Oculus is a pull** then play
  ([music.md](./music.md#commands-pull-playback-back-when-another-device-holds-it)).
  To a device in the cluster it is `spclient().transfer(from, to)`, from the
  active device (or this one when none is): the request Spotify's own apps
  send, with no rate limit. To any other id, the Web API's `PUT
  /v1/me/player` with `{"device_ids": [id], "play": true}`, through the same
  gate and status mapping.

## Gotchas

- `token_provider().get_token` (keymaster) answers 403 — the Web API token
  is Login5's.
- Spotify pads search, playlist and device lists with `null` entries — parse
  items as optional.
- The cluster arrives only on a change — keep the last push and fall back to
  the Web API before it.
- A Web API device id need not match the Connect id — `is_this` also matches
  the name "Oculus".

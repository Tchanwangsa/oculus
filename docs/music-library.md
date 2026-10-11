# Music: playlists, search and Connect devices

What Spotify Connect playback alone does not show: the account's playlists,
search, and the account's devices with a way to move between them. All of it
comes over the session librespot already holds; Spotify's public Web API is
never called. Playback itself is [music.md](./music.md).

## Where

| Piece | Location |
| --- | --- |
| The library: rate-limit gate, the one spclient request, playlists, search | `app/src-tauri/src/music/library/` |
| The rootlist and a playlist's header, decoded | `app/src-tauri/src/music/library/rootlist.rs` |
| The `searchview` request and its answer | `app/src-tauri/src/music/library/search.rs` |
| The Connect cluster, the device list and transfer | `app/src-tauri/src/music/devices.rs` |
| Playlists and search under the player | `app/src/components/music/library/` |
| The device menu | `app/src/components/music/devices/` |

## Under the player, the account's playlists and a search

- **"Your playlists" loads once per mount**, the first time the session is
  connected; Rust answers from memory after its first fetch, so reopening
  the player costs no call. The refresh button beside the heading asks
  Spotify again. Rows show the cover, the name, and owner · N tracks; a
  click plays the playlist (`music_play_context`).
- **Search runs on Enter only**, never per keystroke. Its results replace the
  playlists, grouped Playlists, Albums, Tracks, until the field is cleared
  (its ×, or Escape). A track plays from its place in its album
  (`music_play_track` with the album as context).
- Load and search errors sit inline where the list would be.

## Every library call is one GET to the session's spclient host

`library/spclient.rs` sends `GET <spclient>/<endpoint>` once, with the
session's Login5 token, its client token (when librespot has one) and
librespot's user agent. The host is the spclient access point librespot
resolved for the session. It is blocking `ureq` through
`runtime::blocking::run`, not `SpClient::request`: librespot's HTTP client
sleeps through a 429's `Retry-After` and sends again, retries other failures
up to ten times, and folds every non-2xx into one generic error. Here a
request is sent once, and its status becomes a typed message (401, 403, 429,
other).

| Endpoint | Used for |
| --- | --- |
| `GET /playlist/v2/user/<user>/rootlist?decorate=…&from&length=500` | The playlists, as `SpClient::get_rootlist` asks for them, paged while `truncated`, up to 500 |
| `GET /playlist/v2/playlist/<id>` | A playlist's header when the rootlist carries none for it |
| `GET /searchview/km/v4/search/<query>?entityVersion=2&limit=10&…` | Search, ten of each type |

## The rootlist is decoded with librespot-protocol's playlist4 types

- **It is a `SelectedListContent`** whose `contents` lists items and, when
  there is one per item, meta items carrying each playlist's name, owner
  username, length and picture. Meta items that do not line up are ignored.
- **Folders are flattened**: `spotify:start-group:…` and
  `spotify:end-group:…` markers are skipped and the playlists inside keep
  their place. `spotify:user:<u>:playlist:<id>` becomes
  `spotify:playlist:<id>`; a meta item with status 404 or 410 (deleted by
  its owner) drops the playlist.
- **A playlist with no name in the rootlist** gets its own header from
  `/playlist/v2/playlist/<id>`, four at a time, kept in memory per account
  across refreshes. A 403, another status or an unreadable header leaves that
  playlist out; a rate limit or a lost connection fails the whole list.
- **The cover is the custom picture** (`https://i.scdn.co/image/<hex>`), else
  the generated cover Spotify links on `*.scdn.co` (`default` size first),
  else none.

## Search is the `searchview` service Spotify's clients use

The request is the one Spotify-Android-Good's librespot-based client sends
to spclient (`via_searchview`); librespot-java's `SearchManager` sends the
same route over Mercury, and librespot-golang's `SearchResponse` is the
answer's shape: `{"results": {"<type>": {"hits": [...], "total": n}}}`, each
hit a `uri`, `name`, `image` URL and, for albums and tracks, `artists`, a
track also its `album`. Null hits and hits without a usable URI or name are
dropped; only https images are kept. A 400, 403, 404, 405, 410 or 501 reads
"Search isn't available right now", since it means the route itself is gone
or refused.

## A 429 closes every library call until its Retry-After

A 429 closes a gate in memory (`app/src-tauri/src/music/library/gate.rs`) for
its `Retry-After` seconds (30 when it names none; a later deadline is never
shortened). Until then each call fails at once with "Spotify is rate-limiting
Oculus; try again in N s", without touching the network. Nothing retries on
its own.

## The device list is Spotify's pushed cluster

- **Spotify pushes the Connect cluster** (every device, its name, type and
  volume, and the active device's id) over librespot's dealer at
  `hm://connect-state/v1/cluster`, which the Spirc also hears. Each session
  subscribes before its Spirc starts the dealer, and the last push is kept
  until the connection drops. It costs no call, and it gives the snapshot's
  `playing_on`.
- **Until the first push the list is this device alone.** Spotify pushes
  only on a change, and the cluster the Spirc gets back when it announces the
  device stays inside the Spirc. `music_devices` reads the kept push; the
  menu asks once when it opens. Offline devices are left out; Oculus is
  listed first, then the active device, then by name.
- **A transfer to Oculus is a pull** then play
  ([music.md](./music.md#commands-pull-playback-back-when-another-device-holds-it)).
  To another device it is `spclient().transfer(from, to)`, from the active
  device (or this one when none is): the request Spotify's own apps send. A
  device the last push does not list cannot be chosen.

## Gotchas

- `SpClient::request` sleeps through a 429 and retries — library calls go through `library/spclient.rs`.
- Spotify pads search hits with `null` — parse hits as optional.
- `ListItems` has required fields — a test protobuf without `pos` and `truncated` will not encode.
- The cluster arrives only on a change — the menu shows this device alone until the first push.
- A search endpoint carries the query — log a label, never the endpoint.

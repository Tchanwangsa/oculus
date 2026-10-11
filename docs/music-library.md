# Music: playlists, search, the queue and Connect devices

What Spotify Connect playback alone does not show: the account's playlists
and their tracks, search, the queue and what plays next, and the account's
devices with a way to move between them. All of it comes over the session
librespot already holds; Spotify's public Web API is never called. Playback
itself is [music.md](./music.md).

## Where

| Piece | Location |
| --- | --- |
| The library's calls: playlists, contents, rows, covers, search | `app/src-tauri/src/music/library/mod.rs`, `app/src-tauri/src/music/library/tracks.rs` |
| What it keeps per account, and the rate-limit gate | `app/src-tauri/src/music/library/cache.rs`, `app/src-tauri/src/music/library/gate.rs` |
| The one spclient request (GET, or POST of a protobuf) | `app/src-tauri/src/music/library/spclient.rs` |
| The rootlist and a playlist's header, decoded | `app/src-tauri/src/music/library/rootlist.rs` |
| A playlist's tracks, decoded and paged | `app/src-tauri/src/music/library/contents.rs` |
| Track metadata batches, decoded into rows | `app/src-tauri/src/music/library/metadata.rs` |
| A cover made from a playlist's tracks | `app/src-tauri/src/music/library/covers.rs` |
| The `searchview` request and its answer | `app/src-tauri/src/music/library/search.rs` |
| Queueing, refused while inactive | `app/src-tauri/src/music/commands.rs` (`music_add_to_queue`), `app/src-tauri/src/music/control.rs` |
| Up next in the snapshot | `app/src-tauri/src/music/state.rs`, `app/src-tauri/src/music/events.rs`, `app/src-tauri/src/music/devices.rs` |
| The Connect cluster, the device list and transfer | `app/src-tauri/src/music/devices.rs` |
| Up next, playlists, a playlist's tracks and search under the player | `app/src/components/music/library/` |
| Track rows fetched a page at a time | `app/src/hooks/music/useTrackRows.ts` |
| The device menu | `app/src/components/music/devices/` |

## Under the player, up next, the account's playlists and a search

- **"Your playlists" loads once per mount**, the first time the session is
  connected; Rust answers from memory after its first fetch, so reopening
  the player costs no call. The refresh button beside the heading asks
  Spotify again. Rows show the cover, the name, and owner · N tracks.
- **A playlist without a picture gets a cover after the list**: the page
  asks `music_playlist_covers` for every playlist with none, and covers
  swap in as they arrive. Four URLs draw as a 2×2 grid inside the row's
  rounded square (tiles unrounded, no gaps, as Spotify draws it), one as a
  plain cover, none as the note glyph (`PlaylistCover`).
- **A click opens the playlist** in place of the list: "Your playlists"
  becomes a back button, which puts the list's scroll position back. The
  header shows the cover, name, owner · N tracks and Play
  (`music_play_context`). The tracks are a virtual list (TanStack Virtual,
  48 px rows), since a playlist runs to thousands: `music_playlist_tracks`
  sends every URI at once, and `useTrackRows` asks `music_tracks` for the
  hundred-track pages the list shows as they scroll into view. A row shows
  the album cover, title, artists · album and duration; a click plays the
  track from its place in the playlist (`music_play_track` with the
  playlist as context); the playing track is in brand colour. Local files
  and episodes are left out; a track Spotify describes nothing for reads
  "Unavailable track".
- **Every track row offers "Add to queue"** (a playlist's and search's): an
  icon shown on the row's hover, a `RowAction` inside the row's button. It
  turns to a brand tick for 1.5 s once queued; a refusal shows as the
  player's error, like any command.
- **Up next** sits above the playlists while the snapshot's `up_next` has
  anything: queued tracks under Queued, then the context's under Next,
  their rows from `music_tracks`. The rows are not controls: librespot has
  no command to skip to one.
- **Search runs on Enter only**, never per keystroke. Its results replace the
  playlists (or the open playlist), grouped Playlists, Albums, Tracks, until
  the field is cleared (its ×, or Escape). A track plays from its place in
  its album (`music_play_track` with the album as context).
- Load, track-list and search errors sit inline where the list would be. A
  cover that cannot be made keeps the glyph silently (Rust logs it).

## Every library call is one request to the session's spclient host

`library/spclient.rs` sends `GET <spclient>/<endpoint>`, or a POST of a
protobuf body with `Content-Type: application/x-protobuf` (what
`SpClient::request_with_protobuf` sets), once, with the session's Login5
token, its client token (when librespot has one) and librespot's user
agent. librespot's own client also appends `product=0`, `country` and a
cache-busting `salt` to each URL; these requests do not. The host is the spclient access point librespot
resolved for the session. It is blocking `ureq` through
`runtime::blocking::run`, not `SpClient::request`: librespot's HTTP client
sleeps through a 429's `Retry-After` and sends again, retries other failures
up to ten times, and folds every non-2xx into one generic error. Here a
request is sent once, and its status becomes a typed message (401, 403, 429,
other).

| Endpoint | Used for |
| --- | --- |
| `GET /playlist/v2/user/<user>/rootlist?decorate=…&from&length=500` | The playlists, as `SpClient::get_rootlist` asks for them, paged while `truncated`, up to 500 |
| `GET /playlist/v2/playlist/<id>` | A playlist's header when the rootlist carries none for it; its tracks (`SpClient::get_playlist`) |
| `GET /playlist/v2/playlist/<id>?from&length=500` | The rest of a playlist whose answer is `truncated` |
| `POST /extended-metadata/v0/extended-metadata` | Track rows, `TRACK_V4` for up to 100 tracks per request (`SpClient::get_extended_metadata`) |
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
  else none, and then one is made (below). A playlist's `images` holds one
  URL, four, or none.

## A playlist's tracks are its own answer's items

- **`/playlist/v2/playlist/<id>` is a `SelectedListContent` too**, its
  `contents.items` the entries in order. Only `spotify:track:<22 chars>`
  entries are kept; local files, episodes and anything else are skipped.
- **The plain request comes first.** librespot pages only the rootlist; when
  a playlist's answer says `truncated`, the rest is asked for with the
  rootlist's `from`/`length`, 500 at a time, up to Spotify's 10,000. A page
  that does not start at `from` (the parameters ignored) or is empty ends
  the list there, and a failed later page keeps what was read.
- **Contents are kept per account until a refresh.** The header fetch above
  keeps the first page it read, so a playlist whose header was fetched
  costs no second request for its cover. A cover reads only the first
  page; opening the playlist reads the rest.

## Track rows come from one batched metadata request per hundred

- **`POST /extended-metadata/v0/extended-metadata`** carries a
  `BatchedEntityRequest`: one `EntityRequest` per track, each asking
  `ExtensionKind::TRACK_V4`, no header, as `SpClient::get_metadata` builds
  it. The answer, a `BatchedExtensionResponse`, wraps a `metadata::Track` in
  each entity's `extension_data`.
- **A row** is the track's name, artists' names, album name, duration and
  the album's cover: `album.cover_group` (what librespot reads; `cover`
  when empty), the `DEFAULT` size (300 px), else the largest, as
  `https://i.scdn.co/image/<file id hex>`. The album's gid tells albums
  apart.
- **Rows are kept per account across refreshes.** A track Spotify sent
  nothing for, or one in a batch answered 403, another status or an
  unreadable body, is remembered as having no row. A batch cut short by a
  rate limit or the connection is not remembered, and `music_tracks` fails
  so the page can ask again; the batches before it are kept.

## A playlist without a picture gets a cover from its tracks

What Spotify's own apps do: the album covers of its first tracks. For the
playlists the page asks about, the first page of each is read four at a
time, their first 20 tracks go into one run of metadata batches together,
and each playlist takes its first four distinct albums' covers as a grid,
else its first track's album cover alone, else none (an empty playlist, or
no track with a cover). A playlist Spotify will not describe gets none. All
of these are kept per account and filled into `music_playlists`' answer, so
reopening the player asks nothing; a refresh drops them. A playlist left
unfinished by a rate limit or a lost connection is left out of the answer
and made the next time it is asked for.

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

## Queueing works only while Oculus plays

`music_add_to_queue` sends `Spirc::add_to_queue` while Oculus is the active
device. librespot's Spirc drops it on an inactive device, and it has no
command to queue on another device, so the command is refused there with
the device's name rather than pulling playback here
([music.md](./music.md#commands-pull-playback-back-when-another-device-holds-it)).

## Up next comes from the Spirc while Oculus plays, and the cluster otherwise

- **While Oculus plays**, the Spirc's `SetQueue` player events (on with
  `ConnectConfig::emit_set_queue_events`) set `up_next` from its next
  tracks; it sends one when a context loads and when the queue changes, not
  when it moves on, so a track starting drops itself and everything before
  it from the list.
- **A cluster push** sets it from the active device's
  `player_state.next_tracks`, whichever device that is; a push with no
  device active clears it, one without a player state leaves it.
- **Each entry is a URI and `queued`** (Spotify's provider `queue`), the
  first 20 catalogue tracks; episodes, local files and delimiters are left
  out. Names come from `music_tracks`.

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
- librespot finds `music_play_track`'s track by URI among the context's loaded tracks — one past a page the resolver has not loaded yet starts the playlist from its top.
- The Spirc sends no `SetQueue` when it moves to the next track — `Snapshot::advance_to` trims `up_next` on `TrackChanged`.
- A virtual list's row array is new every snapshot — memoise `useTrackRows`' URIs by their joined key, or every snapshot asks again.

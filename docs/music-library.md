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
| The one spclient request (GET, POST of a protobuf or JSON, PUT of JSON), shared with remote commands | `app/src-tauri/src/music/library/spclient.rs` |
| The rootlist and a playlist's header, decoded | `app/src-tauri/src/music/library/rootlist.rs` |
| A playlist's tracks, decoded and paged | `app/src-tauri/src/music/library/contents.rs` |
| Track metadata batches, decoded into rows | `app/src-tauri/src/music/library/metadata.rs` |
| A cover made from a playlist's tracks | `app/src-tauri/src/music/library/covers.rs` |
| The pathfinder `searchDesktop` request and its answer | `app/src-tauri/src/music/library/search.rs` |
| Queueing, here or on the device playing elsewhere | `app/src-tauri/src/music/commands.rs` (`music_add_to_queue`), `app/src-tauri/src/music/control.rs`, `app/src-tauri/src/music/remote/` |
| Skipping to and removing a track up next | `app/src-tauri/src/music/queue.rs` (`music_skip_to`, `music_queue_remove` in `commands.rs`), `app/src-tauri/src/music/remote/command.rs` |
| Up next and the context in the snapshot | `app/src-tauri/src/music/state/`, `app/src-tauri/src/music/events.rs`, `app/src-tauri/src/music/devices.rs` |
| The Connect cluster, the device list and transfer | `app/src-tauri/src/music/devices.rs` |
| Up next, playlists, a playlist's tracks and search under the player | `app/src/components/music/library/` (up next: `UpNext.tsx`, `RemoveAction.tsx`; search: `SearchGroups.tsx`, `SearchPages.tsx`; an opened playlist or album: `PlaylistView.tsx`) |
| Track rows fetched a page at a time | `app/src/hooks/music/useTrackRows.ts` |
| The rest of the context after Spotify's next tracks | `app/src/hooks/music/useContextRest.ts`, `app/src/lib/music/contextRest.ts` |
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
  header shows the cover, name, owner · N tracks, Play and Shuffle play (an
  outline icon button; `music_play_context` in order or shuffled). The
  tracks are a virtual list (TanStack Virtual, 48 px rows), since a playlist runs to thousands: `music_playlist_tracks`
  sends every URI at once, and `useTrackRows` asks `music_tracks` for the
  hundred-track pages the list shows as they scroll into view. A row shows
  the album cover, title, artists · album and duration; a click plays the
  track from its place in the playlist (`music_play_track` with the
  playlist as context); the playing track is in brand colour. Local files
  and episodes are left out; a track Spotify describes nothing for reads
  "Unavailable track".
- **Every track row offers "Add to queue"** (a playlist's, search's and up next's): an
  icon shown on the row's hover, a `RowAction` inside the row's button. It
  turns to a brand tick for 1.5 s once queued; a refusal shows as the
  player's error, like any command.
- **Up next is behind the queue button** (`QueueToggle`, before the device
  button on `/music`): while it is on, it replaces the library with every
  track the snapshot's `up_next` lists (up to librespot's 80 while Oculus
  plays): queued tracks, then the context's, labelled Queued and Then only
  while something is queued, or "Nothing up next." It is a virtual list
  like a playlist's (48 px rows, labels as items of their own), its rows
  fetched a hundred at a time by `useTrackRows` as they scroll into view.
  The library stays mounted under it, so an open playlist and a search
  survive.
- **An up next row plays on a click** (`music_skip_to`, the row's place in
  `up_next`), and offers "Add to queue" and "Remove from queue"
  (`RemoveAction`, `music_queue_remove`) on hover. A removed row hides at
  once and stays hidden until `up_next` changes; a refusal shows as the
  player's error.
- **After Spotify's list, Up next carries on with the rest of the playing
  playlist or album**, as Spotify's desktop queue does: `useContextRest`
  fetches the snapshot's `context_uri` tracks once (`music_playlist_tracks`
  or `music_album_tracks`) and appends, under Then, everything after the
  last non-queued `up_next` track's place in it (its first place after the
  current track's when it repeats, else its first). Nothing is appended
  while the device playing shuffles, for any other context (an artist,
  Liked Songs), before the tracks arrive, or when that track is not in the
  context. These rows page as they scroll like the rest, so a playlist of
  thousands scrolls to its end. Spotify does not list them, so a click
  plays the track from its place in the context (`music_play_track`) and
  they offer "Add to queue" but no Remove.
- **One list scrolls at a time.** Narrow, the page is one column and the
  list caps at `max-h-96`; from `@3xl` of the page's own width the player
  takes a left column and the list fills the height on the right.
- **Search runs on Enter, or once typing pauses for 500 ms** on a query of
  more than three characters; never per keystroke, and a pause after Enter
  does not ask again. Its results replace the
  playlists (or the open playlist) until the field is cleared (its ×,
  Escape, or deleting the text). Under All they are grouped Songs, Albums,
  Playlists, four of each with a "Show all". When more than one group found
  something, `PillTabs` beside the heading narrow to one group, and a new
  search starts at All. Rust also parses artist hits; nothing shows them
  yet.
- **One group's filter loads more as it scrolls** (`SearchPages`): the first
  ten from `music_search`, then a page of 20 from `music_search_more` each
  time the end of the list comes into view, until Spotify names no next
  page. A failed page shows its error and Try again rather than asking on
  its own.
- **A search track plays** from its place in its album (`music_play_track`
  with the album as context). **An album or playlist opens** to its tracks
  in the same view a playlist of the account's uses, with Play and Shuffle
  play; Back ("Results for …") returns to the results, which stay mounted
  under it, so
  their scroll position and loaded pages survive. An album's tracks come
  from `music_album_tracks`.
- Load, track-list and search errors sit inline where the list would be. A
  cover that cannot be made keeps the glyph silently (Rust logs it).

## Every library call is one request to the session's spclient host

`library/spclient.rs` sends `GET <spclient>/<endpoint>`, or a POST of a
protobuf body with `Content-Type: application/x-protobuf` (what
`SpClient::request_with_protobuf` sets), or, for search only, a POST of
JSON to pathfinder's full URL. Each is sent once, with the session's Login5
token, its client token (when librespot has one) and librespot's user
agent. librespot's own client also appends `product=0`, `country` and a
cache-busting `salt` to each URL; these requests do not. The host is the spclient access point librespot
resolved for the session. It is blocking `ureq` through
`runtime::blocking::run`, not `SpClient::request`: librespot's HTTP client
sleeps through a 429's `Retry-After` and sends again, retries other failures
up to ten times, and folds every non-2xx into one generic error. Here a
request is sent once, and its status becomes a typed message (401, 403, 429,
other). A non-2xx answer's first 512 bytes are logged as
`[music] refused <status>`, since the typed error keeps only the status.

| Endpoint | Used for |
| --- | --- |
| `GET /playlist/v2/user/<user>/rootlist?decorate=…&from&length=500` | The playlists, as `SpClient::get_rootlist` asks for them, paged while `truncated`, up to 500 |
| `GET /playlist/v2/playlist/<id>` | A playlist's header when the rootlist carries none for it; its tracks (`SpClient::get_playlist`) |
| `GET /playlist/v2/playlist/<id>?from&length=500` | The rest of a playlist whose answer is `truncated` |
| `POST /extended-metadata/v0/extended-metadata` | Track rows, `TRACK_V4` for up to 100 tracks per request (`SpClient::get_extended_metadata`); an album's track list, `ALBUM_V4` for one album |
| `POST https://api-partner.spotify.com/pathfinder/v2/query` | Search: `searchDesktop` for ten of each type, `searchTracks`, `searchAlbums` or `searchPlaylists` for a page of 20 of one; the one host that is not spclient |

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
- **An album's tracks** are the same request for the album asking
  `ALBUM_V4`: its `metadata::Album` lists the tracks' gids disc by disc,
  turned into `spotify:track:` URIs, and the rows come from `music_tracks`
  like a playlist's. Not kept: an album is one request each time it opens.
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

## Search is pathfinder's `searchDesktop`, named by a hash that rotates

Spotify's own search service on spclient (`searchview`) answers 400 with an
empty body, so search is the request Spotify's web player and desktop app
send: a POST of the persisted GraphQL query `searchDesktop` to pathfinder,
with the same Login5 token and client token. The query is named only by the
SHA-256 of its document (`SEARCH_DESKTOP` in `search.rs`), which changes when
a web-player release changes the search query. Pathfinder then answers 200
with a `PersistedQueryNotFound` error (or a 412), shown as "Search needs an
update". Every other GraphQL error is shown as unreadable, with its messages.
More of one group is that group's own operation (`searchTracks`,
`searchAlbums`, `searchPlaylists`) with an `offset`, answered with that group
alone and `pagingInfo.nextOffset`, null on the last page. Each has its own
hash, so a release can outdate one and not the others.

To find the current hashes: fetch `https://open.spotify.com`, take its
`web-player.<hash>.js` bundle from `open.spotifycdn.com`, find the
`xpui-routes-search` chunk in its chunk map, and in that chunk read the hash
after each `"<operation>","query",`.

The answer is `{"data": {"searchV2": {"tracksV2", "artists", "albumsV2",
"playlists"}}}`.
A track hit is wrapped as `{"item": {"data": …}}`, the others as
`{"data": …}`, and a hit whose `__typename` is not its group's (`NotFound`,
`RestrictedContent`) is dropped. So are null hits and hits without a usable
URI or name. Each hit takes the smallest https image at least 300 px wide.
A 400, 403, 404, 405, 410 or 501 still reads "Search isn't available right
now", because it means the route is gone or refused.

## Queueing adds to the device playing

`music_add_to_queue` sends `Spirc::add_to_queue` while Oculus is the active
device. While another device plays, it sends that device Spotify's remote
`add_to_queue` command instead, never pulling playback here; with nothing
playing anywhere it is refused with "Play something first, then add to the
queue" ([music.md](./music.md#commands-control-the-device-playing-wherever-it-is)).
Either way the queued track shows in up next from the next queue event or
cluster push.

## Up next comes from the Spirc while Oculus plays, and the cluster otherwise

- **While Oculus plays**, the Spirc's `SetQueue` player events (on with
  `ConnectConfig::emit_set_queue_events`) set `up_next` from its next
  tracks and `can_prev` from its previous ones. The vendored Spirc sends one
  whenever either changes: a load (in its final, shuffled order), a
  transfer, the queue, shuffle, repeat, and every move to the next or
  previous track ([music.md](./music.md#librespot-is-pinned-to-a-dev-branch-commit)).
  A track starting changes nothing by itself.
- **A cluster push** sets it from the active device's
  `player_state.next_tracks`, whichever device that is; a push with no
  device active clears it, one without a player state leaves it.
- **Each entry is a URI and `queued`** (Spotify's provider `queue`), every
  catalogue track listed; episodes, local files and delimiters are left
  out. Names come from `music_tracks`.
- **`context_uri`** comes from the same two places: the `SetQueue` event's
  context while Oculus plays, the active player state's `context_uri`
  otherwise; it clears with `up_next`.

## Skipping to and removing a track up next

With nothing active both are refused with "Play something first".

- **While Oculus plays, the Spirc does both**, through two commands the
  vendored librespot-connect adds ([music.md](./music.md#librespot-is-pinned-to-a-dev-branch-commit)).
  The entry goes as its URI and which of that URI's tracks it is in
  `up_next` (`queue::nth`, from 0); the Spirc finds that track in its own
  next tracks, else the first with the URI, and does nothing when none has
  it. `skip_to` drops the next tracks before it (delimiters join the
  previous ones, as librespot's `next_track` does) and moves on as Next
  does; `remove_next` removes it. Each ends with a `SetQueue` event, so
  `up_next` follows.
- **Another device gets Spotify's remote commands** (`remote/`), built from
  its player state in the last cluster push (`devices::Cluster::queue`): its
  next and previous tracks whole (uri, uid, provider, metadata) and
  `queue_revision`. A push without a player state keeps the last one for
  the same device; nothing active, or the connection dropping, clears it.
  The entry is the one at `index` among the catalogue tracks, or, when the
  list moved since the snapshot, the first with that URI; none is "That
  track is no longer up next". `skip_next` names that track (`uri`, and
  `uid` and `provider` when listed); `set_queue` sends every next track but
  that one (episodes and delimiters too), the previous tracks unchanged, and
  the kept revision.

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
  subscribes before its Spirc starts the dealer, and the last cluster is
  kept until the connection drops. It costs no call, and it gives the
  snapshot's `playing_on`.
- **The announce's answer fills it from the connect on.** Spotify pushes
  only on a change, but it answers the Spirc's announce of the device (on
  connect, and again on each dealer reconnect) with the whole cluster. The
  vendored `Spirc::announced` hands that out
  ([music.md](./music.md#librespot-is-pinned-to-a-dev-branch-commit)), and
  `devices::follow_announced` folds it in exactly as a push
  (`Fold::cluster`). `music_devices` reads the kept cluster; the menu asks
  once when it opens. Offline devices are left out; Oculus is
  listed first, then the active device, then by name.
- **A transfer to Oculus is a pull** then play, the same as "Play here"
  ([music.md](./music.md#commands-control-the-device-playing-wherever-it-is)).
- **A cluster also carries the active device's player state**, which becomes
  the snapshot's `remote` while that device is another one
  ([music.md](./music.md#the-snapshots-remote-is-the-other-devices-playback)).
  To another device it is `spclient().transfer(from, to)`, from the active
  device (or this one when none is): the request Spotify's own apps send. A
  device the last cluster does not list cannot be chosen.

## Gotchas

- `SpClient::request` sleeps through a 429 and retries — library calls go through `library/spclient.rs`.
- Spotify pads search hits with `null` — parse hits as optional.
- A search error that is not an HTTP status arrives as HTTP 200 with an `errors` array — check it before reading `data`.
- `ListItems` has required fields — a test protobuf without `pos` and `truncated` will not encode.
- The cluster is pushed only on a change — the announce's answer (`Spirc::announced`) is what fills the list at connect.
- A search endpoint carries the query — log a label, never the endpoint.
- librespot finds `music_play_track`'s track by URI among the context's loaded tracks — one past a page the resolver has not loaded yet starts the playlist from its top.
- Stock librespot sends no `SetQueue` on a move, a shuffle or a repeat, and a shuffled load's arrives unshuffled — the vendored Spirc sends one after each; never trim `up_next` on `TrackChanged`, which can arrive before or after it.
- A virtual list's row array is new every snapshot — memoise `useTrackRows`' URIs by their joined key, or every snapshot asks again; up next's key carries `queued` too, or a track queued with the same URIs keeps its old group.
- `set_queue` replaces the previous tracks too — send the kept `prev_tracks` back unchanged, or the device's history is wiped.
- Never send Oculus's own device a remote `skip_next` naming a track — librespot's `handle_next` loops forever when that track is not next, hanging the Spirc (no play or pause, then Spotify drops the device); and its queue events lack each track's `context_index`, so they cannot go back in a `set_queue`. Hence the vendored `skip_to` and `remove_next`.

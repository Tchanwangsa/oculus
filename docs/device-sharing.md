# Device sharing

A shared canvas can be opened and edited from a browser on another device on
the same network (an iPad with a Pencil, a phone) through a small HTTP and
WebSocket server in the app. The server is a transport into the canvas
authority ([drawing-canvas.md](./drawing-canvas.md#a-shared-canvas-has-one-writer)),
which stays the only writer of the file. Sharing is off at launch.

## Where

| Piece | Location |
| --- | --- |
| Lifecycle: start, stop, revoke, the `companion-state` payload | `app/src-tauri/src/companion/core.rs` |
| Tauri commands and the event | `app/src-tauri/src/companion/commands.rs`, `app/src-tauri/src/companion/wire.rs` |
| Share registry: ids, secrets, change logs, sockets | `app/src-tauri/src/companion/grants/` |
| Bind, accept, routing, Host/Origin and key checks, WebSockets | `app/src-tauri/src/companion/server/` |
| Authority adapter: events in, commits, snapshots and catch-up | `app/src-tauri/src/companion/bridge/` |
| Browser recovery storage and reload records | `app/src/lib/companion/indexedStorage.ts`, `app/src/lib/companion/drafts.ts` |
| The browser bundle under `/c/` | `app/src-tauri/src/companion/assets/` |
| This Mac's addresses as endpoints, labels, the Host list | `app/src-tauri/src/companion/endpoints/` |
| The address observer it follows | `app/src-tauri/src/runtime/network/` |
| Every bound and timeout | `app/src-tauri/src/companion/limits.rs` |
| The share dialog: state, link, QR, address choice, Stop | `app/src/components/canvas/CanvasShareDialog.tsx`, `app/src/components/canvas/share/` |
| Command wrappers, the `companion-state` store (`useSharing`), the dialog's controller and its registry | `app/src/lib/sharing/` |
| The QR encoder and its checks | `app/src/lib/qr/`, `app/tests/lib/qr/`, `app/tests/qr/` |
| Settings → Device sharing | `app/src/pages/settings/integrations/DeviceSharingPage.tsx`, `app/src/components/settings/sharing/` |
| The companion page's build at each dev start | `app/scripts/build-companion.mjs` |

The selected address's link is one compact input group with copy and, on
macOS, a Share icon that opens the system share sheet. The sheet's destination
picker and dismissal are managed by AppKit; the canvas dialog remains open
behind it.

## The server runs only while something is shared

`companion_share_start(fileId)` needs the canvas's authority session to be
live (the pane attaches first) and otherwise fails with `not-live`. The first
share binds the server; stopping the last share stops it; a second
`share_start` for a shared file returns the same share. Stopping a share ends
only that grant. The authority ending its session (close, delete, a vanished
file) revokes the file's share with reason `ended`. App exit ends every share
(`shutdown`) before the authority closes its sessions.

| Command | Returns |
| --- | --- |
| `companion_share_start(fileId)` | `ShareInfo` `{fileId, shareId, epoch, port, generation, endpoints, clients, phase}`, or `{code, message}` with `code` `not-live`, `server` or `random` |
| `companion_share_stop(fileId)` | nothing; not shared is a no-op |
| `companion_stop_all()` | nothing |
| `companion_shares()` | every live `ShareInfo` |
| `companion_endpoints()` | `{port, generation, endpoints}`, links without a share; re-reads the addresses first |

An endpoint is `{id, url, address, family, interface, label, source,
generation, connectedClients, reachable}`. In a `ShareInfo` its `url` is the
share link `http://<address>:<port>/c/#<shareId>.<secret>`; from
`companion_endpoints` and the event it is the bare page, and `null` while
nothing listens. `id` is `<interface>/<address>`, stable while that address
stays on that interface, so a chosen endpoint survives a list that changes
around it. `connectedClients` counts the live sockets whose Host named that
address (in a `ShareInfo`, that share's; elsewhere, every share's), and
`reachable` is true only while it is above zero: the app never guesses that
a device can reach an address.

The event `companion-state` goes to the main webview after every change,
including a change of address list: `{running, port, ipv6, generation,
endpoints, shares: [{fileId, shareId, epoch, clients, phase}]}`. Its
endpoints are bare pages: it carries no link and no secret;
`companion_shares` does.

The server runs on its own two-worker tokio runtime, not Tauri's, with every
task under one cancellation token and task tracker. Shutdown cancels the tree,
gives sockets up to 0.8 s to send their goodbyes, then drops the runtime; with
idle clients it takes well under a second.

## Ports and addresses

The server prefers port 43821, so links stay the same across launches, then
tries the next 19. It binds `0.0.0.0` (required) and `[::]` on the same port,
v6-only, when that binds. On Unix it sets `SO_REUSEADDR`, which on macOS lets
a wildcard bind succeed beside a listener on `127.0.0.1` alone, so before each
port a loopback connect probe skips ports something already answers on.

Endpoints come from the address observer
([architecture.md](./architecture.md#one-address-observer)): up, not
loopback, not link-local (169.254.0.0/16, fe80::/10); IPv6 ones only while
an IPv6 listener is bound. `generation` is the observer's and moves when the
list changes. Labels name the interface neutrally (`Wi‑Fi/Ethernet en0`,
`VPN/tunnel utun9` for `utun*`, `tun*`, `wg*`, `ppp*` and other tunnels,
`Network …`): an address in 100.64.0.0/10 is not called Tailscale unless the
interface is named so too (`tailscale0`, `Tailscale`), because plenty of
ordinary networks hand out that range.

## Network changes

While the server runs it subscribes to the observer. When the list changes
(Wi‑Fi to a hotspot, a VPN up or down):

- `companion-state` goes out with the new `generation` and endpoints, and
  every share's links, read again through `companion_shares`, are built
  from the same grant: same share id, same key, new addresses.
- The listener, its port, open sockets, requests in flight, the change log
  and the canvas are untouched; nothing is revoked or rotated.
- The Host allow-list is the current list: a Host whose address has gone is
  refused, and one that has just come up is accepted, at once if it is in
  the latest snapshot, otherwise after a re-read the miss forces (at most
  every 250 ms).
- The app re-reads the list when a window comes to the front and when the
  Share dialog asks for endpoints, in case the watcher missed a change.

What cannot follow: a device that only knows a vanished address cannot
discover the new one (there is no mDNS name and no relay). Its socket dies
with the old network, and it rejoins from the new link or QR code, under a
new origin. A VPN such as Tailscale works when the user runs it; its address
is listed like any other, and the app neither installs nor drives one.

## The wire protocol

The key is the link's fragment, `<shareId>.<secret>`, which browsers never
send to a server or in a Referer. The page sends it only as
`Authorization: Bearer <shareId>.<secret>` or in the WebSocket `hello`.

| Request | Answer |
| --- | --- |
| `GET /c/`, `GET /c/assets/**` | The browser bundle (public; Host check only) |
| `GET /api/s/<id>/snapshot` | The canvas file text with its exact `Content-Length`, `X-Oculus-Epoch`, `X-Oculus-Revision`, `X-Oculus-Phase` |
| `POST /api/s/<id>/commit` | `{clientId, seq, opId, epoch, transaction}` → `{epoch, revision, seq, opId, replayed, transaction}` |
| `GET /api/s/<id>/changes?from=<rev>[&epoch=<e>]` | `[{epoch, revision, clientId, transaction}]` after `from` |
| `GET /api/s/<id>/ws` | The push socket |

- Commits are the only write path; none ride the WebSocket. A commit's
  `clientId` reaches the authority as `web:<clientId>`, and change entries
  carry that name, so a device never poses as a desktop pane. A retry with the
  same `clientId` and `seq` gets the original ack (`replayed: true`, no
  transaction), from the authority's receipts.
- Errors are `{code, message}`. Authority codes map to statuses:
  `conflict-*` and `stale-op` 409, `invalid` 400, `session-ended` 410,
  `too-large` 413, `external-edit` 423, `io` 500. Transport codes:
  `unauthorized` 401, `forbidden` 403, `not-found` 404, `method` 405,
  `gone` 410 (catch-up out of range), `busy` 503.
- Each share keeps a change log of the last 256 revisions or 64 MiB, whichever
  is smaller, each entry serialized once. `changes` answers 410 `gone` when
  `from` is older than the log, newer than the head, or `epoch` differs; the
  client then refetches the snapshot.
- The socket's first frame must be `{type: "hello", clientId, secret}` (the
  whole fragment is accepted as `secret` too). The server answers
  `{type: "welcome", epoch, revision, clients}` and then only pushes:
  `change` (an entry of at most 32 KiB, inline), `revision` (pull
  `changes`), `presence`, `state` (`live`, `paused`) and `session-ended`
  before it closes. Its `reason` is a code, `stopped`, `ended` or
  `shutdown`, which the page turns into its own sentence. Client frames
  after `hello` only keep it alive.
- Every 20 s the server sends a protocol Ping and a text `{type: "ping"}`:
  browsers never show protocol pings to page code, so the page's own dead
  timer counts text frames. The page answers `{type: "pong"}`, which, like
  any client frame, only counts as liveness.

## The security boundary

- Host must be one of this Mac's current addresses, or a loopback name, with
  the bound port; a browser's Origin, when sent, must be `http://<Host>`.
  This stops DNS rebinding and other sites' pages. Anything else is 403.
- Only GET and POST are served, so a cross-site preflight fails (405).
- Share ids are 16 random bytes, secrets 32 (`ring`), compared in constant
  time. A secret has a redacting `Debug` and no `Display`; it reaches no log,
  error, event or URL path. A revoked share's key answers 410 for the last 32
  revoked shares, 403 after that.
- Stopping a share revokes its grant, sends `session-ended`, and cancels its
  requests and sockets, including a snapshot mid-transfer.
- The bundle is served from an allow-list: `index.html` and `assets/**`,
  plain path characters only, a known-key set when embedded, and a canonical
  root check (symlinks followed) on disk, with a fixed MIME per extension
  (`application/wasm` for WebAssembly). Pages get `nosniff`, `no-referrer`,
  `no-store` and no framing; hashed assets cache for a year.

## Limits

| Bound | Value |
| --- | --- |
| Request head | 16 KiB, read within 10 s (also the idle keep-alive wait) |
| Commit body | 80 MiB, refused from `Content-Length` before reading; 30 s without progress ends it |
| Connections | 128 |
| Sockets before `hello` | 8 across all shares, 5 s each |
| Sockets per share | 16 |
| Socket frames from a client | 64 KiB |
| Ping (protocol and text) / dead | every 20 s / after 45 s of silence |
| Socket writes | 5 s; goodbye frames 250 ms |

## End-to-end check

Each side's unit tests run against its own fake of the other
(`app/tests/lib/companion/fakeShareServer.ts`,
`app/src-tauri/src/companion/tests/client.rs`). One ignored Rust test runs
the real pair, because it needs bun:

```sh
cd app/src-tauri
cargo test --lib companion::tests::e2e -- --ignored --nocapture
```

`companion/tests/e2e.rs` starts the loopback harness (real server and
authority over a temporary canvas, pings every 200 ms), shares the canvas and
runs `app/tests/e2e/shareClientE2E.ts` with bun, passing the port and key in
`OCULUS_E2E_PORT` and `OCULUS_E2E_KEY`. The script drives real `ShareClient`s
built by `browserShareEnv` over bun's fetch, WebSocket and timers, sending
`Origin` as a browser does; one tab uses `127.0.0.1`, the other `localhost`.
The test plays the desktop: a stdout line `@@ <command> <json>` is answered
on stdin with `@@ok <json>`. It is not a `*.test.ts`, so `bun test` skips it.
In about 6 s it covers:

- Both tabs load the snapshot and converge on each other's strokes and on a
  desktop commit; a foreign Origin is refused on POST and on the socket.
- A fresh desktop JavaScript session rejoins the existing native authority,
  then receives browser edits live and sends desktop edits back live, without
  whole-file writes or reopening the canvas.
- Independent edits merge; the same shape edited on both is a `conflict` for
  one tab, whose draft stays until `useSharedCanvas()`.
- A 10 MiB PNG goes up as one POST and down to the other tab intact.
- Deleting the large image and immediately undoing it while the delete is held
  before sending restores the image on both tabs and in the Mac's saved file.
- A commit whose answer is lost is retried as the same op and replayed once.
- A socket closed under a tab reconnects; an edit made meanwhile is
  `offline-unsaved`, then saved once. Text pings keep both sockets alive.
- Stop (share stop, then the authority closes) ends both tabs with their
  drafts exportable; a reload of the old link, a retry and the old op sent to
  the restarted share replay nothing into the new epoch.

The test then checks the file on disk: a valid canvas with each stroke once,
the image byte for byte (SHA-256), thirteen revisions in the first epoch and one in
the second, desktop and `web:` committers, and no secret in the script's
output. A LAN address is not covered: the harness's addresses are fake, so
both tabs use loopback.

## Recovery survives a browser reload without moving the drawing

The browser keeps unconfirmed edits, the in-flight operation identity and a
complete drawing for download in IndexedDB, scoped to the page origin and share
ID. LocalStorage records migrate only after the IndexedDB transaction commits.
Embedded PDF pages stay in the recovery record; there is no small character
limit that drops them. Writes run in order, and an operation is recorded before
it is sent, so a lost reply and reload can resend the same operation safely.
The sent operation, its observed revision, and edits made after it are stored
separately; an immediate undo survives a delayed confirmation and reload. A kept
conflict also survives reload as a downloadable draft until explicitly resolved.
Loading waits for recovery before connecting. Earlier epochs stay downloadable
and are never replayed into a new share. Actual storage failures retain the draft
in memory and show a warning while changes remain unconfirmed.

The header has a fixed height, and notices overlay the canvas instead of adding
rows above it. Saving, reconnecting and storage notices cannot change the
canvas's position or size while the person writes. The Mac owns the saved file;
its separate disk recovery record is described in [drawing-canvas.md](./drawing-canvas.md#one-save-session-owns-every-pane-editing-a-file).

## What sharing does not promise

- LAN and VPN only: no relay, no TLS, no mDNS name. A device must reach one of
  the listed addresses.
- A changed address is not followed by a device: one that only knows a
  vanished address cannot find the Mac again; the Mac shows the new link
  and the device rejoins under a new origin, whose drafts do not follow.
- A shared notebook edits like any canvas, and a page added on the device reaches
  the Mac. Creating a notebook, copying a PDF into one and exporting it as a PDF
  are desktop-only: the device's page has no library and no save dialog.
- Replay is guaranteed only inside a live epoch. Stopping and restarting a
  share makes a new key.

## Desktop UI

A canvas's header has a Share button that opens `CanvasShareDialog`. Sharing
outlives the dialog; the header's status says `Sharing · 2 devices` while it
runs. `app/src/lib/sharing/` is desktop-only (it calls Tauri), and
`app/tests/repo/sharingGraph.test.ts` keeps the companion from importing it.

- **One controller per session** (`shareController` in `controller.ts`, handed
  out by `shareControllerFor` in `registry.ts`), over injected commands so its
  tests run on fakes. Start is `session.attachAuthority()` then
  `companion_share_start`; if the share fails, an authority it attached is
  detached again and the `{code, message}` shows inline. Stop is
  `companion_share_stop` then `detachAuthority()`. A second click joins the
  action under way. Actions resolve with their failure, never reject.
- **Opening the dialog rereads** `companion_shares` and `companion_endpoints`
  (which rereads the addresses first), and so does focusing the window or
  showing it again while the dialog is open. Reading never starts or stops a
  share.
- **`companion-state` has one listener for the app's life** (`store.ts`),
  seeded from the commands until the first event, read through
  `useSharing(fileId?)`. The registry passes every event to every controller:
  its bare endpoints become the view's addresses, client counts and phase
  update the view, a new `generation` or client count rereads the share's
  links (and each address's own device count, which only `companion_shares`
  carries), and a share missing from an event is checked with `companion_shares`
  before the session detaches, because an event can predate the start. A
  vanished share gets a controller even for a canvas whose dialog never
  opened, so a share stopped in Settings or ended natively hands its file back
  to ordinary saves.
- **The state line claims only what the server counts**: `Sharing · no device
  connected` until `clients` moves. An address row adds `N devices connected
  via this address` only from its `connectedClients`.
- **The address choice** (`pickEndpoint` in `view.ts`) keeps the user's pick
  while the share lists its id (`<interface>/<ip>`, stable across rereads).
  Without a manual choice it prefers an address a device is connected through,
  then an IPv4 endpoint with a link. The link
  field selects on focus and copies with inline Copied feedback. On macOS,
  **Share link…** opens the native share sheet (including AirDrop) for that selected
  address, using an NSURL so the full fragment key survives. The user chooses
  the destination in macOS; opening or cancelling the sheet sends nothing.
  The action opens on mouse-down (or keyboard activation), as AppKit requires.
  One interaction stays active until it ends; further presses cannot stack
  pickers over an AirDrop sheet. The retained delegate supplies the Oculus
  source window. AirDrop reports its result while its sheet is still up, and
  the sheet's Done button needs the picker, service and delegate alive, so they
  stay retained until the window's sheet ends (`NSWindowDidEndSheetNotification`).
  Closing the dialog or changing the selected link ends any interaction without
  an open sheet; an open sheet keeps its own macOS controls.
  Failures appear beside the action; transfer feedback belongs to macOS.
  The bridge is `shell/sharing/` and is restricted to the main webview.
- **The QR code is drawn locally** by `app/src/lib/qr/`: ISO/IEC 18004 byte
  mode, level M, versions 1–10 (up to 213 bytes), the mask with the lowest
  penalty, an SVG path with the 4-module quiet zone, black on white in every
  theme. `app/tests/qr/coreImage.test.ts` renders ten link-shaped strings to
  PNG and decodes them with macOS's CoreImage detector (`decode.swift`); it
  skips without swift.
- **Stop asks inline**, in the dialog and in Settings; Escape cancels the
  question before it closes the dialog.
- **Settings → Device sharing** says what sharing is and is not, and lists the
  live shares from the event: the canvas title from its open session, the
  device count, the addresses folded (the event's bare endpoints, so no link
  or key is shown), Stop, and Stop all.
- **`tauri dev` never builds `dist/companion/`**, so the dev preflight runs
  `app/scripts/build-companion.mjs`. It skips when `index.html` there is newer
  than `app/src/`, `canvas-core/pkg/` and the build's config (all of `src/`,
  since Tailwind scans it); a build takes about 2 s, and a failed one is
  logged without stopping the start.

## Gotchas

- The authority's event callback runs under its session lock: it may touch
  the grants only; closing connections and stopping the server go to a
  thread (`Core::end_file`).
- `tauri dev` does not build the bundle: run `bun run companion:build` once in
  `app/`. A debug build serves `app/dist/companion` from disk; a missing
  bundle is a 404 saying so, never the app's own `index.html` (Tauri's
  resolver falls back to it, so embedded lookups check the bundle's keys first).

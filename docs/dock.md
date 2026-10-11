# The messaging dock

WhatsApp, Instagram DMs, Messenger and Discord, each the service's real web app
in a native child WKWebView of the main window. Every service is off until
turned on in Settings → Messaging; a service that is on has one page for the
life of the app, signed in through its own persistent WebKit data store.
Unread counts badge the rail and the service tabs, and an in-memory Inbox
lists each message the pages report while Oculus is open.

## Where

| Piece | Location |
| --- | --- |
| Service table, state, `dock-state` snapshot | `app/src-tauri/src/shell/dock/services.rs`, `app/src-tauri/src/shell/dock/mod.rs` |
| Page creation, data store setup, close | `app/src-tauri/src/shell/dock/page.rs` |
| Place, park, resize | `app/src-tauri/src/shell/dock/layout.rs` |
| Handing keyboard focus back to the app | `app/src-tauri/src/shell/dock/focus.rs` |
| Commands | `app/src-tauri/src/shell/dock/commands.rs` |
| Capability invariant test | `app/src-tauri/src/shell/dock/capabilities_test.rs` |
| Unread from the title | `app/src-tauri/src/shell/dock/title.rs` |
| The `dock` message handler and its origin check; parsing a message | `app/src-tauri/src/shell/dock/channel.rs`, `app/src-tauri/src/shell/dock/message.rs` |
| Injected scripts: Notification shim, chat-list row watcher | `app/src-tauri/src/shell/dock/scripts.rs`, `app/src-tauri/src/shell/dock/scripts/` |
| The Inbox (memory only) | `app/src-tauri/src/shell/dock/inbox.rs` |
| Frontend service table, invoke wrappers, path helpers | `app/src/lib/dock/index.ts` |
| Enabled set (`dock_enabled` setting) and the `dock-state` mirror | `app/src/stores/shell/dockPrefsStore.ts`, `app/src/stores/shell/dockStore.ts` |
| Boot: re-enable each service, follow `dock-state` and `dock-inbox` | `app/src/hooks/shell/useDockPages.ts` (mounted in `AppLayout`) |
| The `/dock` page: header tabs, Inbox, a service's slot | `app/src/pages/tools/DockPage.tsx`, `app/src/pages/tools/dock/DockHeader.tsx`, `app/src/pages/tools/dock/Inbox.tsx`, `app/src/pages/tools/dock/useDockViewport.ts` |
| Rail item, Settings page | `app/src/components/sidebar/Sidebar.tsx`, `app/src/components/settings/web/MessagingSection.tsx` |

## The frontend owns which services are on; Rust owns the pages

The `dock_enabled` setting (a JSON array of ids) is the only record of which
services are on. At startup `useDockPages` calls `dock_set_enabled(id, true)`
for each one. That call is idempotent, so a webview reload re-runs it
harmlessly; enabling creates the page and disabling closes it. A page is never
recreated just to show it.

Dock state is separate from the in-app browser's. Labels are `dock-<id>`, the
snapshot goes out as `dock-state` (to the `main` webview only), and no dock
page ever enters `BrowserState`, because `useBrowserTabs` adopts every page in
that list as a tab.

Messaging is one rail item, under a rule after the sections with Spotify's
tile below it, and shown while any service is on; its badge is the unread
total. It opens `/dock`, the Inbox. `DockPage`'s header has an Inbox tab, then
a tab per enabled service with its own count; switching replaces the route
(`/dock` or `/dock/<id>`), and leaving a service parks its page. A tab on a
dock route is titled with the service's name, never the page title, which
carries unread counts. ⌘R on a dock route reloads that service.

## A page is placed or parked, never hidden

- **Shown** = moved over the slot `DockPage` leaves in the content card, then
  `show()`. The slot is measured with the browser's `measure()`
  ([viewers.md](./viewers.md#the-in-app-browser-is-a-native-page-per-tab-owned-by-rust)), so page zoom is handled.
- **Parked** = moved fully left of the window at the slot's own size. A
  hidden page throttles its timers to about 1 Hz; a parked one keeps its
  sockets and timers at full rate, and its title updates keep arriving. A
  sliver would flip these sites to their mobile layouts.
- A page parks when its tab leaves the front, when a popover covers the slot
  (nothing draws over a native view), and when the route unmounts. Two panes
  on one service share one page; only the pane that last placed it may move
  or park it.
- Rust re-lays out the shown page on window resize, and resizes parked ones
  to the new slot size.
- The service tabs sit in a header row above the slot, which also keeps the
  card's top corners clear: `round_corners` rounds only a page's bottom ones.

## Placing never takes keyboard focus

Pages are built with `.focused(false)`, and `dock_place` never calls
`set_focus()`; a click in the page focuses it. Parking or closing a page that
holds first responder hands it to the app's own WKWebView (`focus.rs`, using
the address `shell/keys.rs` records), or keystrokes would go to a page nobody
can see. While a dock page holds focus it gets ⌘-keys before the menu, like a
browser page ([shell.md](./shell.md#the-shell-owns-tabs-side-panels-and-every-window-shortcut)).

## One data store per service, exempt from ITP

- `data_store_identifier` takes a 16-byte id derived from the service id
  (`store_id`). **Changing an id signs the user out of that service for
  good.**
- WebKit keeps stores under `~/Library/WebKit/<executable name>/WebsiteDataStore/<uuid>/`,
  keyed by the executable, not the bundle id. The dev build (`app`) and the
  bundled app have separate logins.
- ITP is on in a non-browser WKWebView: a site's script storage is wiped 30
  *operating days* (days the app ran) after the last click or key press in
  it. WhatsApp (IndexedDB) and Discord (localStorage) logins live there. Each
  store therefore gets the private `_setPersistedSites:` with the service's
  sites, guarded by `respondsToSelector:` (macOS 15.2+). WebKit matches by
  registrable domain.
- Per-service stores need macOS 14. Below that wry silently uses the shared
  default store; `configure_store` logs it and leaves that store's ITP alone.
- `configure_store` also gives each store the WebCrypto master key delegate,
  so the `CryptoKey`s WhatsApp and Messenger keep in IndexedDB are unwrapped
  with the key keyd serves, not WebKit's keychain item, whose prompt returns
  with every dev rebuild ([auth.md](./auth.md#webkits-webcrypto-master-key-comes-from-keyd-per-build)).
- Messenger is hosted at `www.facebook.com/messages/`: its session cookie
  lives on `.facebook.com`, which messenger.com doesn't read. A fresh store
  asks for the Messenger PIN to restore end-to-end-encrypted history; the
  user types it.

## Popups go to the default browser

`on_new_window` opens http(s) URLs with the opener plugin and denies the
popup, so a login that relied on a popup would land in the browser. The
logins seen so far use iframes (captchas) and same-page navigations (2FA).
No navigation inside a page is blocked.

## Unread counts come from the page title

`on_document_title_changed` keeps firing while a page is parked or the window
is minimised, and every service puts its count first: `(2) WhatsApp`,
`(356) Discord | Friends`, `(4) Instagram • Messages`,
`(20+) Messenger | Facebook`. `title.rs` reads the leading `(N)`/`(N+)`.
WhatsApp and Instagram count unread chats, Discord unread messages, and
Facebook's number includes its other notifications.

## The Inbox hears from two read-only scripts

Both are injected into each page's main frame (`scripts/`) and post to a
`dock` WKScriptMessageHandler. Neither changes what the page shows.

- **Notification shim** (all four): replaces `window.Notification` and
  reports it as granted, so a site that notifies (Discord, once its own
  "Enable Desktop Notifications" is on) hands over sender and text. Nothing
  is displayed. Instagram never calls it.
- **Chat-list row watcher** (Instagram, WhatsApp, Messenger): a
  MutationObserver on the inbox list with per-site selectors. It posts when a
  row turns unread, an unread row's preview changes, or a new unread chat
  appears at the top. The first rows after load are a silent baseline. It
  reads leaf `textContent`, since a parked page has no layout for
  `innerText`. Instagram's preview becomes `N new messages` after the first
  unread one. The selectors assume the English UI.

`channel.rs` accepts a message only from the page's main frame, over https,
on one of the service's domains, and parses the body as hostile
(`message.rs`: strings only, length-capped). The page itself can post to the
handler too, so a site could add Inbox rows of its own; that is all a message
can do.

The Inbox (`inbox.rs`) keeps the newest 200 items in memory, so message text
never reaches the disk or `oculus.db`, which agents can read. Duplicates are
dropped for ten minutes (a notification by its tag, a row by name and
preview), and a row and a notification from the same sender within ten
seconds count once. Clicking an item opens its service, never the chat, so
reading the Inbox sends no read receipt.

## No capability may reach a dock page

Tauri injects its IPC bridge into every child webview, remote pages included,
so only the capability ACL stands between a dock page and the app's commands
([architecture.md](./architecture.md#the-main-window-holds-one-webview-per-browser-tab-and-messaging-service)).
`capabilities_test.rs` fails the build on any capability, in
`app/src-tauri/capabilities/` or inline in `tauri.conf.json`, with a `remote`
section, a `webviews` entry that could match `dock-*`, or a `windows` entry
that could match `main` (dock pages are children of `main`, and window and
webview scopes match by OR). Globs are read so that they can only over-match.

## Gotchas

- `hide()` on a dock page throttles it — park it off-screen instead.
- Putting a dock page in `BrowserState.tabs` makes `useBrowserTabs` adopt it as a browser tab.
- A new `store_id` or a renamed executable orphans every login.
- Creating a page activates the app (wry), so it brings Oculus to the front when a service is turned on.
- Message text in a setting, a table or a log puts private messages where agents can read them — keep the Inbox in memory.
- A capability scoped to window `main` or a `dock-*` glob hands every command to a remote site; the test catches it.

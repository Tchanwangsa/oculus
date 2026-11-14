# Companion input diagnostics

An optional local receiver keeps timestamped browser traces on the Mac so a
reported freeze can be matched to input, drawing and save activity. It runs
independently of Oculus and needs no desktop restart.

## Where

| Piece | Location |
| --- | --- |
| Start the receiver, validate batches, rotate the log | `app/scripts/companion-diagnostics.ts` |
| Capture browser inputs, lifecycle, slow operations and errors | `app/src/companion/diagnostics/index.ts` |
| Isolated browser input comparison pad | `app/input-test.html`, `app/src/companion/diagnostics/inputTest.ts` |
| Independent worker watchdog and bounded upload queue | `app/src/companion/diagnostics/worker.ts`, `app/src/companion/diagnostics/protocol.ts` |
| Optional tracing hooks shared with the desktop, the log and the overlay | `app/src/lib/canvas/diagnostics.ts` |
| Gesture and tool decisions, camera moves, paint timing | `app/src/hooks/canvas/useCanvasInteraction.ts` |
| The debug button's on-screen touch, Pencil and gesture overlay | `app/src/companion/DebugToggle.tsx`, `app/src/companion/gestureOverlay.ts` |
| Rebase, operation persistence and drawing serialization timing | `app/src/lib/companion/shareClient.ts`, `app/src/lib/companion/drafts.ts` |

## A local receiver enables tracing on the next page refresh

From `app/`, run `bun scripts/companion-diagnostics.ts <repo-root>` with an
absolute repo root. The receiver listens on port 43841, on the Mac's LAN
interfaces, and writes `data/logs/companion-input.jsonl` beneath that root.
`OCULUS_DIAGNOSTICS_PORT` overrides the port (zero selects an available port).
It publishes `app/dist/companion/assets/diagnostics-config.json` with that
port and a temporary diagnostic token, and restores the config after a
companion rebuild. SIGINT or SIGTERM stops it and removes its config.

Build the companion with `bun run companion:build` and refresh its iPad page.
The browser checks for the config at startup. Without it, the diagnostic worker,
input listeners and heartbeat timers are absent. This config is a static asset;
its filename and browser requests contain no sharing secret.

The receiver accepts batches only with its temporary token and a browser Origin
on the sharing port range, on the same hostname as the receiver request. It
allows no log reads. Request bodies are bounded to 64 KiB and 100 events; at
most eight requests wait for disk writes. Logs rotate at 10 MiB with three
previous files, independently of canvas recovery storage.

## Capture, handler and state records distinguish different freezes

The trace records pointer down/up/cancel, pointer capture changes, clicks,
double-clicks, touch and native gesture boundaries, selection/menu events,
wheel activity and command keys. Selection events record their type, not text.
Pointer moves, raw touch moves and wheel events are sampled, with aggregate
counts. Passive window-capture and
document-bubble records show whether an input reached the page and continued
through its handlers; touch records include contact counts and cancellation
state. Native gesture records include scale, rotation and whether their default
was cancelled. Startup identifies the bundle, touch policy and native gesture
policy, including whether the canvas touch guard is attached to the body.
Visual viewport changes record browser page scale and offsets separately
from the drawing's camera. Input delay compares
the event timestamp with delivery time. Control labels, inline body pointer
blocking and the canvas's inert state help identify an overlay intercepting
input. Editable text, document contents, image data and sharing secrets are
excluded.

Gesture records include the active tool, pointer identity, touch count,
Pencil ownership and rejected inputs. Tool requests and committed tool state are
separate events. Share records include status, revision, pending-operation count,
blocked state and commit start/end, without operation contents.

Painting, pointer handlers, document diff/application, reconciliation,
serialization and storage have named timing spans. Synchronous operations over
32 ms and asynchronous storage operations over 250 ms record their duration and
caller stack. JavaScript errors and rejected promises record their error stacks.
Hidden source maps are emitted beside the companion bundle for local decoding
of these stack locations.

## Camera records explain an unexpected scroll

Every pan or two-finger navigation that ends records `camera.pan`: its source
(`touch-pan`, `pen-pan`, `two-finger`, `two-finger-cancel`), whether it glided
and why not (`verdict`: `few-samples`, `held`, `short-span`, `short-travel`,
`pinch`, `slow`), its travel, the release speed, how late the release was
handled, and up to 80 camera samples as `ms,dx,dy` from the pan's start. A
glide ends with `camera.glide-end` (`rest`, `pinned`, or `halted:` and what
stopped it) and how far it moved. Any single camera write of 150 px or a
quarter's zoom records `camera.jump` with its cause (pan, glide, wheel, page,
fit, zoom, resize, external, publish), and a canvas size change records
`canvas.resize`. `gesture.down` carries the contact's width and height.

## The debug button shows gestures for a screen recording

The bug button beside Expand on the drawing page turns an overlay on and off,
remembered on that device and independent of the receiver. It draws a ring
under each finger (red when the system cancels the touch), a filled dot under
the Pencil and a hollow one while it hovers, and a panel in the bottom-left
corner: a clock to match the video to the log, then the last few decisions —
what each contact became when it lifted, cancels, a second finger turning a
gesture into navigation, ignored input, tool changes, pan releases and why they
did or didn't glide, glide ends, camera jumps (two-finger ones aside, which a
pinch makes on every frame) and resizes. It never takes input. Both it and the
log listen to the same canvas traces, so either works without the other.

## A worker detects a blocked main thread without waiting for it to recover

The main thread sends a heartbeat every 250 ms. While the page is visible, a
worker reports a missing heartbeat after one second, including operations that
were active when communication stopped. It can upload that report while the
main thread remains blocked. The main thread also records delayed animation
frames and timer callbacks when execution resumes. Hidden pages suppress the
watchdog; visibility changes reset the frame and timer baselines.

These are instrumented operation and error stacks, not a sample of Safari's
suspended native or JavaScript stack. A frozen browser process or suspended
worker cannot send live reports; frame gaps and buffered events provide evidence
when it resumes. A responsive event loop with ignored inputs has different
records from a synchronous stall.

Every five seconds a `page.health` record confirms frame and timer progress,
with their largest gaps, active pointer contacts and observed pointer captures.
These positive records show whether the page stayed responsive during a period
with no delivered input.

Each event has the device's wall-clock milliseconds; each log line also has the
Mac's receive time. Startup records the device timezone and offset. Match a
reported local time against those timestamps and session IDs, accounting for any
device clock difference. Uploads have a bounded queue and timeout; a disconnected
receiver drops the oldest records rather than blocking drawing or growing
memory without limit.

## A separate pad compares input without opening a notebook

`/c/assets/input-test.html` is a standalone entry served by the same sharing
server. Open it in another tab on the device. It paints only temporary ink and
counts pointer contacts, raw touch starts and test-button taps. Its import graph
excludes React, the canvas editor, the sharing client and notebook storage.
Nothing drawn here is saved or sent as document content.

The pad uses the same optional diagnostic receiver and a fresh diagnostic
identity, without a sharing credential. Startup records `page: input-test`, its
touch policy and native gesture policy so its input and health traces can be
compared with `page: drawing` sessions. It cancels native touch defaults only on
its pad; ordinary buttons keep browser defaults. Test-button taps and clearing
the pad produce `probe.tap` and `probe.clear` records. Closing the tab discards
the temporary ink.

## Gotchas

- Refresh the iPad page after enabling the receiver; an existing page does not install diagnostics retroactively.
- Keep the receiver running while reproducing; stopping it removes the config and prevents further uploads.
- A diagnostic failure must never change a drawing handler's result or exception.
- An event absent from the trace may have been withheld by WebKit before JavaScript could observe it.

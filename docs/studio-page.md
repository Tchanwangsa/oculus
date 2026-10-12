# The Studio page

The rail's Studio section: describe a video, watch an agent make it in a
thread beside its player, render the final, browse every video, and try the
components a video is built from. The
renderer, the commands and the thread link are [studio.md](./studio.md); this
page is the frontend over them.

## Where

| Piece | Location |
| --- | --- |
| Routes, the nav column's width and fold, the page header row | `app/src/routes.tsx`, `app/src/layouts/StudioLayout.tsx`, `app/src/components/studio/StudioHeader.tsx` |
| The nav column: New video, Videos, Components, the Recent list and its search | `app/src/components/studio/nav/` |
| Pages | `app/src/pages/studio/` (`NewVideoPage`, `VideosPage`, `VideoPage`, `VideoThreadPage`, `ComponentsPage`) |
| New video: the box and its start-up | `app/src/components/studio/new/NewVideo.tsx`, `app/src/hooks/studio/useNewVideo.ts`, `app/src/lib/studio/title.ts` |
| The style chip: theme, format, length, voice | `app/src/components/studio/style/`, `app/src/hooks/studio/useThemes.ts`, `app/src/lib/studio/themes.ts` |
| The thread column | `app/src/components/studio/thread/ThreadColumn.tsx` |
| Player, render bar, scene strip, script, facts, the captions switch, the Export menu | `app/src/components/studio/video/` (`ExportMenu.tsx`, `CaptionsSwitch.tsx`), `app/src/hooks/studio/useVideoPlayback.ts`, `app/src/hooks/studio/useVideoDetail.ts`, `app/src/lib/studio/exports.ts` |
| The Videos grid, its cards and subject pills | `app/src/components/studio/videos/`, `app/src/components/ui/layout/MediaGrid.tsx` |
| Stills (`studio_frame`, `studio_thumbnail`) as object URLs | `app/src/lib/studio/stills.ts`, `app/src/hooks/studio/useStill.ts` |
| Render state, the video list, the event bridge | `app/src/stores/studio/` (`renderStore.ts`, `videosStore.ts`), `app/src/hooks/backend/useStudioEvents.ts` |
| The Components playground: list, stage, timeline, inspector, source, change box | `app/src/pages/studio/ComponentsPage.tsx`, `app/src/components/studio/components/` (`inspector/` for the props form) |
| The playground's hooks: component list, source, stage frames, playhead, change flow | `app/src/hooks/studio/` (`useComponents`, `useComponentSource`, `useStageFrame`, `useStageWidth`, `usePlayhead`, `useComponentChange`, `useDefaultModel`), `app/src/lib/studio/props.ts`, `app/src/lib/studio/changeRequest.ts` |
| Paths | `app/src/lib/studio/routes.ts` |

## Six routes under one nav column

`/studio` is New video, `/studio/videos` the grid, `/studio/videos/:slug` a
video's detail, `/studio/videos/:slug/thread` the conversation that made it,
and `/studio/components` and `/studio/components/:name` the playground. `StudioLayout` owns the nav column's panel (`useResizablePanel`, ⌘⌥B and
each page header's toggle, as Chat's column) and reads the video list once;
a page puts its own `StudioHeader` row over its body. A video's title rides
in `?n=` (`videoHref`) because `tabInfo` titles a tab from its path alone,
and falls back to the slug. There is no Reels or Themes page.

The Recent list is the newest eight videos, or every title the column's
search matches. A row opens the thread when the video has one, else the
detail. It shows a spinner while the video is rendering or its thread is
mid-turn, and the rail's Studio item shows a brand dot while any render runs;
there are no toasts or bars ([ui.md](./ui.md)).

## New video makes the folder, then the thread

`NewVideo` is Chat's `Composer` with two additions every caller shares: a
`leading` slot, chips after the subject's above the box, and a
`placeholder`. Its chips are the subject (this page's own choice, not
Chat's "next thread" scope) and the style chip. Sending runs
`useNewVideo`:

1. `newVideo({title, subject, theme?, portrait})`. The title is the prompt's
   first line without its `@` paths, cut at 60 characters; the thread names
   itself later and the slug comes from the title. The subject is the course
   code without its term (`displayCode`), the name of its folder under
   `studio/subjects/`.
2. `harnessSend(null, provider, text, {model, reasoningEffort, subjectId,
   studio: {slug, lengthSecs, format}})`, which links the new thread to the
   video ([studio.md](./studio.md#a-video-has-a-thread)).
3. Reload the thread and video lists, and open `/studio/videos/:slug/thread`.

A failure at either step shows its sentence under the box and gives the text
back (`Composer`'s `restore`). One after step 1 leaves the scaffold behind, a
video with no thread in the grid.

**The style chip** holds the theme, the format (Video or Reels, which is
`portrait`) and the length target (30 s, 60 s, 2 min, 5 min). The theme is
null until picked by hand, which sends none, so the backend gives the video
its subject's default theme; picking one holds across a subject change. The
format and length are remembered (`oculus-studio-style`). Themes come from
`studio_themes` for the chosen subject; if the command rejects, the list is
only "Default" and the popover prints the reason. The voice is shown read-only
from Settings → Voice with a link there. There is no sample button: a sample
would be a billed TTS call, and nothing in Settings makes one.

## The thread page: the conversation beside the video

`ThreadColumn` is the thread's timeline and composer as Chat draws them
(`Timeline`, `useThreadActions`, `useStickToBottom`), holding its rows like
the lecture dock does and releasing them on leaving. When a turn ends it
re-reads the video's detail and the list, since the agent may have changed
files. A video with no thread redirects to its detail.

The right column loads `studio_video_detail` and then renders `VideoStage`:

- **The player** is the app's `MediaPlayer` with no dock (`dock` is
  optional) over the newest video render (`draft`, `final`, `small` or `2x`;
  `playableRenders`), `mediaSrc(path)` plus `&v=<mtime>` so a re-render of
  the same path is a new file to the element. With several, pills choose.
  When the video does not burn captions in and the render has its `.vtt`
  beside it, `studio_render_captions` reads it, `parseVtt` makes the cues
  and the bar gets its captions button; otherwise there is none. No render
  shows "Not rendered yet" and Render draft. The scrub bar's notches
  are the scene starts. The element shares the app's one sound with lectures
  and library videos (`useLibraryPlayback`).
- **The scene strip** has a tile per scene, a still from `studio_frame` at
  80% of the scene's length (a second in if there is no narration). A press
  seeks to the scene's start; a scene with no start is only a picture, and
  the scene under the playhead is lit. A still that cannot be made, a stale
  narration, says so once under the strip with the command's own sentence.
- **The render bar** starts `studio_render` through `renderStore`. A render
  in flight, of any preset (the Export menu starts the others), swaps Render
  final for a bar naming the preset, the percentage and Cancel; a failure
  stays until the next render starts. Rendering is local (ffmpeg),
  so unlike Settings it may start here.

## Renders live in a store, not in the page

`renderStore.active` is keyed by the video's absolute folder, holds the
preset, and is fed by
`studio-render-progress` (`useStudioEvents`, mounted with the other backend
bridges) and by `start`, which awaits `renderVideo`. The fraction is the
segments done plus the part of the current one, over all of them. An end,
however it ends, removes the entry, keeps a failure's sentence in `errors`,
bumps `settled[dir]` (a page showing the folder re-reads on it) and reloads
the video list. The store outlives navigation, so the Recent row, the rail
dot, a card's badge and the page's bar all follow one render.

## Videos and the detail

The grid (`MediaGrid`, `MediaTile`) shows each video's thumbnail
(`studio_thumbnail`, 16:9, a portrait one letterboxed rather than cropped),
title and subject, scene count and age. A video that is rendering carries a
badge with its percentage and a bar. One whose video.json will not load shows
the problem in place of its facts. Pills over the grid narrow by subject
(shown when two or more subjects have videos; the choice is remembered in
`oculus-studio-subject`). With no video the page offers New video.

The detail is the same `VideoStage` beside the facts (subject, theme, voice,
length, shape), the captions switch and the script split by scene; a press
on a scene seeks the player. **Continue in chat** opens the thread page.

- **Burn in captions** (`CaptionsSwitch`) writes video.json's `captions`
  through `studio_set_captions` and re-reads the detail; it applies from
  the next render. A failed write shows its sentence under the switch.
- **Export** (`ExportMenu`) is a pill over a popover of the presets
  ([studio.md](./studio.md#rendering-a-video)): each row names the preset and
  what it makes for this video's shape (`presetHint`), and its state from
  `studio_exports`, read when the menu opens and again when a render of the
  folder ends: a check when its file is current, "out of date" when the file
  is from an older version, the reason when it cannot render now. Render (or
  Re-render) closes the menu and starts it through `renderStore`, so its
  progress is the render bar's; while any render of the video runs the
  buttons are off and the running row spins. A row with a file has Show in
  Finder (Show in Explorer on Windows, `revealItemInDir`). A failure to read
  the statuses shows in the popover; a render's failure, under the render
  bar.

## The Components playground

`/studio/components/:name` shows one component of a library; the bare route
opens the first. The page's library select (global, or a subject by course
code, remembered in `oculus-studio-components-library`) decides whose
cascade `studio_components`, `studio_themes` and the frames read
([studio.md](./studio.md#the-playgrounds-commands)); a name the library lacks
says so. Three columns:

- **The list** groups the shells and the components. A row marks a subject's
  own version of a component (`level` is `subject`) and a component that does
  not load (`problem`, its sentence in the row's tooltip).
- **The stage** (`Workbench`) is one component's centre column and inspector.
  The theme select lists `studio_themes` with each one's swatch (a theme that
  does not parse is listed but cannot be picked; the choice is remembered in
  `oculus-studio-components-theme`), the example pills pick a named example,
  and the frame is `studio_component_frame` at the playhead `t`, for a scene
  length of 2, 4 or 8 s, at the stage's width in device pixels (a step of 80,
  320 to 1920). The props it sends are always the inspector's whole set, not
  an example name, so an edit is just a new request.
- **The inspector** is generated from `props`: `str` and `latex` a text
  field, `num` a number field, `bool` a switch, `oneOf` a select of the
  schema's values, `token` a select of the names in the chosen theme's
  `tokens` section (`color`, or a dotted path; a colour shows its dot; a theme
  without the section leaves a plain text field), and `list` rows of the inner
  kind with Add and remove. A prop that is not set takes its default, which
  shows as the placeholder (a switch shows the default's state); an optional
  prop or one with a default has an unset button, and an empty text field on
  an optional prop unsets it. Reset to example restores the chosen example's
  props. Nothing here edits the component.

**One frame in flight.** `useStageFrame` keeps at most one
`studio_component_frame` call running and, behind it, only the newest request,
so play and scrub skip the moments the renderer cannot keep up with and never
queue; the playhead ticks about 20 times a second. An edit to props, theme,
size or length waits 150 ms for the next one, a move of `t` does not. The
last frame that drew stays up (dimmed) when a request fails, with the
command's own sentence under it, and playing pauses so a failing frame is not
asked again every tick. Frames are not cached: each is an object URL, revoked
when the next lands.

**Source and Ask for a change** share the area under the timeline. Source is
`studio_component_source` in a read-only `CodeText`, with the level it came
from. Ask for a change is one box that sends the request to the agent
(`useComponentChange`): the first send of a page session creates a Chat
harness thread (provider and model from the harness store, filled from the
provider's list as the composer does, `useDefaultModel`; `subjectId` from the
library select), later sends continue it, and a different subject starts a
new one since a thread's subject is fixed at creation. The text is
`changeRequestText` (`lib/studio/changeRequest.ts`): a first line naming the
component and its level, the request, then the instruction to use the
oculus-studio skill, make a scratch video, change the component in its
`components/` folder, check it with `gallery` and `promote` it to the same
level. The thread is an ordinary one, so it shows in Chat ("Open thread").
While its turn runs the box shows a spinner and refuses another send; when
the turn ends the page reads the component's source and history again,
reloads the list (so props and examples follow the new text) and redraws
the stage. If a newer snapshot exists at that level the result says the change
was kept and **Undo** writes that snapshot back with
`studio_component_restore` (the text it replaces becomes the newest
snapshot; the page offers no redo). A change at
a level that had no file (a first subject version) has no snapshot to undo to
and says so; a turn that left the text alone says the thread shows why.

## Stills are shared and queued

`stillUrl` turns each still into an object URL, shared by every tile that
shows it, four at a time (each is a render in Rust) with the newest 300
kept. A key names what the picture is of, including the video's modified
time, so a changed video shows its new frame; a rejection is not cached.

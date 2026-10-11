# The Studio page

The rail's Studio section: describe a video, watch an agent make it in a
thread beside its player, render the final, and browse every video. The
renderer, the commands and the thread link are [studio.md](./studio.md); this
page is the frontend over them.

## Where

| Piece | Location |
| --- | --- |
| Routes, the nav column's width and fold, the page header row | `app/src/routes.tsx`, `app/src/layouts/StudioLayout.tsx`, `app/src/components/studio/StudioHeader.tsx` |
| The nav column: New video, Videos, the Recent list and its search | `app/src/components/studio/nav/` |
| Pages | `app/src/pages/studio/` (`NewVideoPage`, `VideosPage`, `VideoPage`, `VideoThreadPage`) |
| New video: the box and its start-up | `app/src/components/studio/new/NewVideo.tsx`, `app/src/hooks/studio/useNewVideo.ts`, `app/src/lib/studio/title.ts` |
| The style chip: theme, format, length, voice | `app/src/components/studio/style/`, `app/src/hooks/studio/useThemes.ts`, `app/src/lib/studio/themes.ts` |
| The thread column | `app/src/components/studio/thread/ThreadColumn.tsx` |
| Player, render bar, scene strip, script, facts | `app/src/components/studio/video/`, `app/src/hooks/studio/useVideoPlayback.ts`, `app/src/hooks/studio/useVideoDetail.ts` |
| The Videos grid, its cards and subject pills | `app/src/components/studio/videos/`, `app/src/components/ui/layout/MediaGrid.tsx` |
| Stills (`studio_frame`, `studio_thumbnail`) as object URLs | `app/src/lib/studio/stills.ts`, `app/src/hooks/studio/useStill.ts` |
| Render state, the video list, the event bridge | `app/src/stores/studio/` (`renderStore.ts`, `videosStore.ts`), `app/src/hooks/backend/useStudioEvents.ts` |
| Paths | `app/src/lib/studio/routes.ts` |

## Four routes under one nav column

`/studio` is New video, `/studio/videos` the grid, `/studio/videos/:slug` a
video's detail and `/studio/videos/:slug/thread` the conversation that made
it. `StudioLayout` owns the nav column's panel (`useResizablePanel`, ⌘⌥B and
each page header's toggle, as Chat's column) and reads the video list once;
a page puts its own `StudioHeader` row over its body. A video's title rides
in `?n=` (`videoHref`) because `tabInfo` titles a tab from its path alone,
and falls back to the slug. There is no Reels, Themes or Components page.

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

- **The player** is the app's `MediaPlayer` with no dock and no captions
  button (`dock` and `captions` are optional) over the newest render,
  `mediaSrc(path)` plus `&v=<mtime>` so a re-render of the same path is a
  new file to the element. With both a draft and a final, pills choose. No
  render shows "Not rendered yet" and Render draft. The scrub bar's notches
  are the scene starts. The element shares the app's one sound with lectures
  and library videos (`useLibraryPlayback`).
- **The scene strip** has a tile per scene, a still from `studio_frame` at
  80% of the scene's length (a second in if there is no narration). A press
  seeks to the scene's start; a scene with no start is only a picture, and
  the scene under the playhead is lit. A still that cannot be made, a stale
  narration, says so once under the strip with the command's own sentence.
- **The render bar** starts `studio_render` through `renderStore`. A render
  in flight swaps Render final for a bar, the percentage and Cancel; a
  failure stays until the next render starts. Rendering is local (ffmpeg),
  so unlike Settings it may start here.

## Renders live in a store, not in the page

`renderStore.active` is keyed by the video's absolute folder and fed by
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
length, shape) and the script split by scene; a press on a scene seeks the
player. **Continue in chat** opens the thread page and **Export mp4** reveals
the final (else the newest render) in Finder with `revealItemInDir`.

## Stills are shared and queued

`stillUrl` turns each still into an object URL, shared by every tile that
shows it, four at a time (each is a render in Rust) with the newest 300
kept. A key names what the picture is of, including the video's modified
time, so a changed video shows its new frame; a rejection is not cached.

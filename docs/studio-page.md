# The Studio page

The rail's Studio section: describe a video, watch an agent make it in a
thread beside its player, render the final, browse every video, try the
components a video is built from, and edit the themes it is drawn in. The
renderer, the commands and the thread link are [studio.md](./studio.md); this
page is the frontend over them.

## Where

| Piece | Location |
| --- | --- |
| Routes, the nav column's width and fold, the page header row | `app/src/routes.tsx`, `app/src/layouts/StudioLayout.tsx`, `app/src/components/studio/StudioHeader.tsx` |
| The nav column: New video, Videos, Components, Themes, the Recent list and its search | `app/src/components/studio/nav/` |
| Pages | `app/src/pages/studio/` (`NewVideoPage`, `VideosPage`, `VideoPage`, `VideoThreadPage`, `ComponentsPage`, `ThemesPage`) |
| New video: the box and its start-up | `app/src/components/studio/new/NewVideo.tsx`, `app/src/hooks/studio/useNewVideo.ts`, `app/src/lib/studio/title.ts` |
| The style chip: theme, format, length, voice | `app/src/components/studio/style/`, `app/src/hooks/studio/useThemes.ts`, `app/src/hooks/studio/useThemeDefaults.ts`, `app/src/lib/studio/themes.ts` |
| The thread column | `app/src/components/studio/thread/ThreadColumn.tsx` |
| Player, render bar, scene strip, the detail's dock (script, scene grid), facts, Render draft, the Export menu | `app/src/components/studio/video/` (`VideoDetailStage.tsx`, `VideoDock.tsx`, `RenderDraftButton.tsx`, `ExportMenu.tsx`), `app/src/hooks/studio/useVideoPlayback.ts`, `app/src/hooks/studio/useDockedFrameHeight.ts`, `app/src/hooks/studio/useSceneStill.ts`, `app/src/hooks/studio/useVideoDetail.ts`, `app/src/lib/studio/exports.ts` |
| The Videos grid, its cards and subject pills | `app/src/components/studio/videos/`, `app/src/components/ui/layout/MediaGrid.tsx` |
| Stills (`studio_frame`, `studio_thumbnail`) as object URLs | `app/src/lib/studio/stills.ts`, `app/src/hooks/studio/useStill.ts` |
| Render state, the video list, the event bridge | `app/src/stores/studio/` (`renderStore.ts`, `videosStore.ts`), `app/src/hooks/backend/useStudioEvents.ts` |
| The Components playground: list, stage, timeline, inspector, source, change box | `app/src/pages/studio/ComponentsPage.tsx`, `app/src/components/studio/components/` (`inspector/` for the props form) |
| The playground's hooks: component list, source, stage frames, playhead, change flow | `app/src/hooks/studio/` (`useComponents`, `useComponentSource`, `useStageFrame`, `useStageWidth`, `usePlayhead`, `useLibraryChange`, `useDefaultModel`), `app/src/lib/studio/props.ts`, `app/src/lib/studio/changeRequest.ts` |
| The library choice both pages share, where an item comes from, own versions | `app/src/hooks/studio/useStudioLibrary.ts`, `app/src/components/studio/LevelMark.tsx`, `app/src/components/studio/components/OwnVersionActions.tsx` |
| The Themes page: list, sample scenes, header actions, token inspector, copies | `app/src/pages/studio/ThemesPage.tsx`, `app/src/components/studio/themes/` (`inspector/` for the token form), `app/src/hooks/studio/useThemeSource.ts`, `app/src/lib/studio/themeDraft.ts` |
| The colour control | `app/src/components/ui/color/`, `app/src/lib/ui/color.ts` |
| Paths | `app/src/lib/studio/routes.ts` |

## Eight routes under one nav column

`/studio` is New video, `/studio/videos` the grid, `/studio/videos/:slug` a
video's detail, `/studio/videos/:slug/thread` the conversation that made it,
`/studio/components` and `/studio/components/:name` the playground, and
`/studio/themes` and `/studio/themes/:name` the Themes page. `StudioLayout` owns the nav column's panel (`useResizablePanel`, ⌘⌥B and
each page header's toggle, as Chat's column) and reads the video list once;
a page puts its own `StudioHeader` row over its body. A video's title rides
in `?n=` (`videoHref`) because `tabInfo` titles a tab from its path alone,
and falls back to the slug. There is no Reels page.

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
`portrait`) and the length target (30 s, 60 s, 2 min, 5 min). Until a theme
is picked by hand the chip shows the subject's default
(`studio_theme_defaults`' `effective`, marked "Default" in the list) and
sends none, so the backend gives the video that default by name; choosing a
subject brings its default. A theme picked by hand holds across a subject
change, and picking the marked default again goes back to following it.
Each theme row says where it comes from (an own version, the subject's,
global, built in). The format and length are remembered
(`oculus-studio-style`). Themes come from `studio_themes` for the chosen
subject; if the command rejects, the list is only "Default" and the popover
prints the reason. The voice is shown read-only
from Settings → Voice with a link there. There is no sample button: a sample
would be a billed TTS call, and nothing in Settings makes one.

## The thread page: the conversation beside the video

`ThreadColumn` is the thread's timeline and composer as Chat draws them
(`Timeline`, `useThreadActions`, `useStickToBottom`), holding its rows like
the lecture dock does and releasing them on leaving. When a turn ends it
re-reads the video's detail and the list, since the agent may have changed
files. A video with no thread redirects to its detail.

The right column loads `studio_video_detail` and then renders `VideoStage`
(the player, the render bar and the scene strip; the detail page composes the
player and render bar itself):

- **The player** is the app's `MediaPlayer` with no dock (the detail page
  gives it one) over the newest video render (`draft`, `final` or `small`;
  `playableRenders`), `mediaSrc(path)` plus `&v=<mtime>` so a re-render of
  the same path is a new file to the element. With several, pills choose.
  When the render has its `.vtt` beside it, `studio_render_captions` reads it, `parseVtt` makes the cues
  and the bar gets its captions button; otherwise there is none (and none
  when the video's captions are drawn into the picture, `detail.captions`).
  No render shows "Not rendered yet", and the header offers Render draft. The scrub bar's notches
  are the scene starts. The element shares the app's one sound with lectures
  and library videos (`useLibraryPlayback`).
- **The scene strip** has a tile per scene, a still from `studio_frame` at
  80% of the scene's length (a second in if there is no narration). A press
  seeks to the scene's start; a scene with no start is only a picture, and
  the scene under the playhead is lit. A still that cannot be made, a stale
  narration, says so once under the strip with the command's own sentence.
  The detail page has no strip: its Scenes tab draws the same stills
  (`useSceneStill`) as a grid.
- **The render bar** is under the player and has no buttons: the render
  pills when there are several, and when the shown one was made. A render in
  flight, of any preset (the header starts them through `renderStore`), takes
  its place with a bar naming the preset, the percentage and Cancel; a failure
  stays until the next render starts. Rendering is local (ffmpeg), so unlike
  Settings it may start from the page. The thread page has no render button
  either: its video is rendered from the detail page.

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

The detail (`VideoDetailStage`) is the player with a dock, and under it a row
of the facts (subject, theme, voice, length, shape) beside the render bar.
**Continue in chat** opens the thread page. The header's last button is
**Render draft** while the video has no playable render (it starts
`studio_render` for `draft` through `renderStore`, and spins, disabled, while
any render of the folder runs), and **Export** once it has one.

- **The dock** is the lecture player's (`MediaDock`, `useTranscriptDock`):
  the player's dock button, T and the dock's × fold it, its header drags it to
  an edge, its divider resizes it, and the side, size and open state are
  `playerPrefsStore`'s, shared with lectures ([viewers.md](./viewers.md)),
  except the side: a video's dock keeps its own (`oculus-studio-video-dock-side`,
  `DockSide` passed to `useMediaPlayer`), on the right until moved. Its
  tabs, **Script** and **Scenes** (the tab is remembered in
  `oculus-studio-video-dock-tab`), take the place of the lecture's Transcript:
  - **Script** (`ScriptPanel`) is the narration split by scene, with each
    scene's length. The scene under the playhead is lit, and the list follows
    it as the lecture transcript does (`FollowList`: nudge, Back to live,
    idle re-sync). A press seeks to the scene's start.
  - **Scenes** (`SceneGrid`) is a grid of the scenes' stills, as many columns
    as the dock's width allows, each with its number and length. The scene
    under the playhead is lit and scrolled into view when the playhead enters
    it, unless the pointer is on the grid. A press seeks.
  - Both are inert while there is no render, since there is no video to move;
    the Script still reads and the stills still draw.
- **The picture** is 16:9 beside or above the dock whatever its width
  (`useDockedFrameHeight` sets the frame's height from the frame's width, the
  dock's drawn size and the dock side), within `100vh - 14rem` and no shorter
  than 18rem; folding the dock gives the picture the whole width, and the
  height eases with the dock's slide. The page is the content's width up to
  1440px. The picture keeps 480px beside a side dock (`dockKeepWidth`): a
  narrower page draws the dock at the bottom without rewriting the preference.
  With no render the picture says "Not rendered yet" under the control bar,
  with no button of its own.
- **Export** (`ExportMenu`) is a pill over a popover of the presets, Draft,
  Final and Small ([studio.md](./studio.md#rendering-a-video)): each row names
  the preset and what it makes for this video's shape (`presetHint`), and its
  state from `studio_exports`, read when the menu opens and again when a
  render of the folder ends: a check when its file is current, "out of date"
  when the file is from an older version, the reason when it cannot render
  now. Render (or Re-render) closes the menu and starts it through
  `renderStore`, so its progress is the render bar's; while any render of the
  video runs the buttons are off and the running row spins. A row with a file
  has Show in Finder (Show in Explorer on Windows, `revealItemInDir`). A
  failure to read the statuses shows in the popover; a render's failure,
  under the render bar.
- **Captions in the picture** is the switch under the presets (hint: "Drawn
  into the frames, so every player shows them"), per video: `detail.captions`
  is video.json's `captions` and `setCaptions` writes it
  (`studio_set_captions`). A change re-reads the detail and the statuses, so
  the files made the other way show "out of date". With the switch on the
  player has no CC button, since the words are in the frames; with it off the
  CC button shows the render's `.vtt`. A failed write shows under the switch.

## The Components playground

`/studio/components/:name` shows one component of a library; the bare route
opens the first. The page's library select (global, or a subject by course
code) decides whose cascade `studio_components`, `studio_themes` and the
frames read ([studio.md](./studio.md#the-playgrounds-commands)); a name the
library lacks says so. The choice is one for this page and the Themes page
(`useStudioLibrary`, remembered in `oculus-studio-components-library`). Three
columns:

- **The list** groups the shells and the components. In a subject's library
  a row says where it comes from (`LevelMark`): "Own version" when the
  subject's file replaces a global one (`shadows`), "Subject" when only the
  subject has it, "Global" when it is inherited. A component that does not
  load is marked (`problem`, its sentence in the row's tooltip).
- **Own versions.** In a subject's library the stage's header offers **Own
  version** on an inherited component, which copies the global text into
  the subject's library (`studio_component_write` at the subject level), and
  **Remove own version** on one that replaces a global component, which
  deletes the copy after a confirm (`studio_component_delete`; the text is
  kept in the library's history) so the subject falls back to the global
  one. Both reload the list and the stage.
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
(`useLibraryChange`): the first send of a page session creates a Chat
harness thread (provider and model from the harness store, filled from the
provider's list as the composer does, `useDefaultModel`; `subjectId` from the
library select), later sends continue it, and a different subject starts a
new one since a thread's subject is fixed at creation. The text is
`changeRequestText` (`lib/studio/changeRequest.ts`): a first line naming the
component and its level, the request, then the instruction to use the
oculus-studio skill, make a scratch video, change the component in its
`components/` folder, check it with `gallery` and `promote` it to the same
level. The Themes page sends a theme's version of the same text. The thread is an ordinary one, so it shows in Chat ("Open thread").
While its turn runs the box shows a spinner and refuses another send; when
the turn ends the page reads the component's source and history again,
reloads the list (so props and examples follow the new text) and redraws
the stage. If a newer snapshot exists at that level the result says the change
was kept and **Undo** writes that snapshot back with
`studio_component_restore` (the text it replaces becomes the newest
snapshot; the page offers no redo). A change at
a level that had no file (a first subject version) has no snapshot to undo to
and says so; a turn that left the text alone says the thread shows why.

## The Themes page

`/studio/themes/:name` opens one theme of the browsed library (the same
library select and choice as the Components page); the bare route opens the
theme new videos there take. **New theme** in the header saves a copy of the
built-in `default` under a name asked for. Three columns:

- **The list** is every theme in effect for the library (`studio_themes`), a
  thumbnail each drawn in CSS from its tokens (`ThemeThumb`: the canvas, a
  heading in the heading font, the muted and accent colours, a surface card
  with its corner), so the open theme's thumbnail follows its unsaved draft.
  "Default" marks the theme a new video here takes
  (`studio_theme_defaults`' `effective`), and in a subject's library
  `LevelMark` says where each comes from.
- **The stage** (`ThemeEditor`): the name and where it lives, the actions,
  then the theme on a sample scene. The scene pills are starter components
  the library has and that load (Title, Graph, Diagram, Code, Chart, Table,
  Grid: `TitleCard`, `Axes`, `Graph`, `Code`, `Bars`, `Table`, `Grid`),
  each in its first example, drawn by `studio_component_frame` through
  `useStageFrame` (one in flight, edits wait 150 ms, as the playground) with
  the timeline to play or scrub its motion. Under it, Ask for a change and
  the theme file (the draft's text while there is one).
- **The inspector** (`inspector/`) edits the draft: **Default for** chips
  (Global, and the browsed subject) that make the theme that library's
  default or clear it (`studio_theme_set_default`, the library's
  `library.json`; a subject's own theme cannot be the global default), then
  every `color.*` token with the colour control, the heading, body and code
  fonts (the families studio-core bundles), each `type.*` font size,
  `radius.*`, the motion durations, easings (four cubic-bezier numbers),
  spring presets (a preset choice, then its stiffness, damping and mass),
  the scene transition's kind, length and direction, magic move, colour
  blend and motion blur, and the narration's lead, tail and pause. There is
  no texture token. A new heading or body font also moves the `type.*`
  entries that used the old family (`setFontRole`, by entry name when two
  roles share one), since components spread `type.*`.

**Edits are a draft until saved.** Each edit replaces the draft
(`lib/studio/themeDraft.ts`, key order kept), and the stage draws it at once
by sending the draft's text as `themeText` in place of the saved theme. The
header then offers **Revert** and **Save**, which writes
`formatTheme(draft)` (two-space JSON) to the theme's own level with
`studio_theme_write`: a global theme browsed from a subject is saved for
every subject, and says so. The built-in `default` cannot change: its Save
is **Save as a copy**, which asks for a name. With no unsaved edit and a
snapshot at the theme's level, **Undo** writes the newest one back
(`studio_theme_restore`; that restore is itself snapshotted). **Duplicate**
saves the text (or the draft) under a new name in the browsed library and
opens it. In a subject's library **Own version** and **Remove own version**
work as on the Components page; **Delete** removes a library theme that
replaces nothing, after a confirm, its text kept in the history. Any write,
an agent's turn or a default change reloads the list, the defaults, the
source and the stage, and a new saved text replaces the draft.

**Ask for a change** is the playground's box over `useLibraryChange` with
the theme's level: the text names the theme's file in its library
(`../studio/library/themes/<name>.json` or the subject's), and tells the
agent to change a copy in a scratch video, check it with `gallery --theme`
and promote it back, so the old text is kept for Undo. It is closed for
the built-in theme (duplicate it first) and while the draft has unsaved
edits.

**The colour control** (`components/ui/color/`) is a round swatch and its
hex that opens a popover: a saturation and brightness pad, a hue strip, an
opacity strip and the hex text. It is pointer-driven (`useFractionDrag`
reports positions as fractions of the element's box, so page zoom does not
matter) and the arrow keys move each part. It holds the hue itself, so
dragging through grey keeps it, and hands back `#rrggbb`, or `#rrggbbaa`
when not opaque ([ui.md](./ui.md) on native colour inputs).

## Stills are shared and queued

`stillUrl` turns each still into an object URL, shared by every tile that
shows it, four at a time (each is a render in Rust) with the newest 300
kept. A key names what the picture is of, including the video's modified
time, so a changed video shows its new frame; a rejection is not cached.

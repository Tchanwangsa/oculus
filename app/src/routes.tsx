import type { ComponentType } from "react";
import { Navigate, Outlet, type RouteObject } from "react-router-dom";
import { LoadingFill } from "@/components/ui/layout/PageParts";
import { RouteError } from "@/components/ErrorBoundary";
import SubjectLayout from "@/layouts/SubjectLayout";
import SettingsLayout from "@/layouts/SettingsLayout";
import RevisionLayout from "@/layouts/RevisionLayout";
import StudioLayout from "@/layouts/StudioLayout";
import TasksLayout from "@/layouts/TasksLayout";
import HomePage from "@/pages/start/HomePage";
import NewTabPage from "@/pages/start/NewTabPage";
import SubjectTasksRedirect from "@/pages/work/SubjectTasksRedirect";
import SubjectsRedirect from "@/pages/start/SubjectsRedirect";

/** Paths stay synchronous for matching and ⌘-click; page modules load only
 *  when a pane visits them, except Home and the new-tab page, which a fresh
 *  pane opens on, and the `/subjects` redirect, which the rail opens. Each
 *  pane builds its own memory router over it
 *  (`app/src/components/tabs/TabPane.tsx`). The shell sits outside them all. */
function PaneRoot() {
  return <Outlet />;
}

/** A route `lazy` for a page module's default export. */
const page = (load: () => Promise<{ default: ComponentType }>) => () =>
  load().then((m) => ({ Component: m.default }));

export const routes: RouteObject[] = [
  {
    path: "/",
    element: <PaneRoot />,
    errorElement: <RouteError />,
    hydrateFallbackElement: <LoadingFill />,
    children: [
      { index: true, element: <HomePage /> },
      // Where the + button and ⌘T land.
      { path: "new", element: <NewTabPage /> },
      { path: "chat", lazy: page(() => import("@/pages/tools/ChatPage")) },
      { path: "revision", element: <RevisionLayout />, children: [
        { index: true, lazy: page(() => import("@/pages/revision/CheatSheetPage")) },
        { path: "review", lazy: page(() => import("@/pages/revision/ReviewPage")) },
        { path: "quizzes", lazy: page(() => import("@/pages/revision/QuizzesPage")) },
        { path: "artifacts", lazy: page(() => import("@/pages/revision/ArtifactsPage")) },
      ] },
      { path: "calendar", lazy: page(() => import("@/pages/tools/CalendarPage")) },
      {
        // One nav column beside the projects and tasks pages.
        element: <TasksLayout />,
        children: [
          { path: "projects", lazy: page(() => import("@/pages/work/ProjectsIndexPage")) },
          { path: "projects/:projectId", lazy: page(() => import("@/pages/work/ProjectPage")) },
          // Nested under the project, whose columns define its statuses.
          { path: "projects/:projectId/tasks/:taskId", lazy: page(() => import("@/pages/work/TaskPage")) },
          { path: "tasks", lazy: page(() => import("@/pages/work/TasksPage")) },
          // An unfiled task: same page, with a null project and the default board.
          { path: "tasks/:taskId", lazy: page(() => import("@/pages/work/TaskPage")) },
        ],
      },
      // No page: reopens the last subject (SubjectsRedirect).
      { path: "subjects", element: <SubjectsRedirect /> },
      { path: "subjects/:subjectId/projects", element: <SubjectTasksRedirect /> },
      { path: "subjects/:subjectId/tasks", element: <SubjectTasksRedirect /> },
      // A file or lecture as a whole page (a tab, or the side panel), outside
      // SubjectLayout's tabs.
      { path: "subjects/:subjectId/file", lazy: page(() => import("@/pages/subject/files/FilePage")) },
      { path: "subjects/:subjectId/lecture", lazy: page(() => import("@/pages/subject/content/LecturePage")) },
      {
        // SubjectLayout resolves the subject once, via outlet context.
        path: "subjects/:subjectId",
        element: <SubjectLayout />,
        children: [
          { index: true, lazy: page(() => import("@/pages/subject/content/OverviewPage")) },
          { path: "modules", lazy: page(() => import("@/pages/subject/content/ModulesPage")) },
          {
            // Sub-tabs are child routes so restore, crumbs and ⌘-click key
            // off the path (see FilesPage).
            path: "files",
            lazy: page(() => import("@/pages/subject/files/FilesPage")),
            children: [
              { index: true, element: <Navigate to="downloads" replace /> },
              { path: "downloads", lazy: page(() => import("@/pages/subject/files/DownloadsPage")) },
              { path: "media", lazy: page(() => import("@/pages/subject/files/MediaPage")) },
              { path: "uploads", lazy: page(() => import("@/pages/subject/files/UploadsPage")) },
              { path: "drive", lazy: page(() => import("@/pages/subject/files/DrivePage")) },
              { path: "documents", element: <Navigate to="../drive" replace /> },
              { path: "canvases", element: <Navigate to="../drive" replace /> },
            ],
          },
          { path: "lectures", lazy: page(() => import("@/pages/subject/content/LecturesPage")) },
          { path: "announcements", lazy: page(() => import("@/pages/subject/content/AnnouncementsPage")) },
          { path: "assignments", lazy: page(() => import("@/pages/subject/content/AssignmentsPage")) },
          { path: "discussion", lazy: page(() => import("@/pages/subject/content/DiscussionPage")) },
          { path: "lessons", lazy: page(() => import("@/pages/subject/content/LessonsPage")) },
          // Legacy paths that restored tabs and Recent entries may carry.
          { path: "downloads", element: <Navigate to="../files/downloads" replace /> },
          { path: "uploads", element: <Navigate to="../files/uploads" replace /> },
        ],
      },
      {
        // Videos: the nav column (`StudioLayout`) beside New video, the
        // grid, one video's detail and its thread, the Components playground
        // and the Themes page.
        path: "studio",
        element: <StudioLayout />,
        children: [
          { index: true, lazy: page(() => import("@/pages/studio/NewVideoPage")) },
          { path: "videos", lazy: page(() => import("@/pages/studio/VideosPage")) },
          { path: "videos/:slug", lazy: page(() => import("@/pages/studio/VideoPage")) },
          { path: "videos/:slug/thread", lazy: page(() => import("@/pages/studio/VideoThreadPage")) },
          { path: "components", lazy: page(() => import("@/pages/studio/ComponentsPage")) },
          { path: "components/:name", lazy: page(() => import("@/pages/studio/ComponentsPage")) },
          { path: "themes", lazy: page(() => import("@/pages/studio/ThemesPage")) },
          { path: "themes/:name", lazy: page(() => import("@/pages/studio/ThemesPage")) },
        ],
      },
      // Spotify's full player, opened beside the page from the rail (docs/music.md).
      { path: "music", lazy: page(() => import("@/pages/tools/MusicPage")) },
      { path: "sync", lazy: page(() => import("@/pages/tools/SyncPage")) },
      // The id names a native WebView Rust places under this content area.
      { path: "browse/:id", lazy: page(() => import("@/pages/tools/BrowserPage")) },
      {
        path: "settings",
        element: <SettingsLayout />,
        children: [
          { index: true, element: <Navigate to="canvas" replace /> },
          { path: "canvas", lazy: page(() => import("@/pages/settings/integrations/CanvasPage")) },
          { path: "appearance", lazy: page(() => import("@/pages/settings/AppearancePage")) },
          { path: "browser", lazy: page(() => import("@/pages/settings/integrations/BrowserPage")) },
          { path: "music", lazy: page(() => import("@/pages/settings/integrations/MusicPage")) },
          { path: "device-sharing", lazy: page(() => import("@/pages/settings/integrations/DeviceSharingPage")) },
          { path: "storage", lazy: page(() => import("@/pages/settings/library/StoragePage")) },
          { path: "agents", lazy: page(() => import("@/pages/settings/integrations/AgentsPage")) },
          { path: "opencode", lazy: page(() => import("@/pages/settings/integrations/OpencodePage")) },
          { path: "jobs", lazy: page(() => import("@/pages/settings/library/JobsPage")) },
          { path: "parsing", lazy: page(() => import("@/pages/settings/library/ParsingPage")) },
          { path: "embeddings", lazy: page(() => import("@/pages/settings/library/EmbeddingsPage")) },
          { path: "transcription", lazy: page(() => import("@/pages/settings/library/TranscriptionPage")) },
          { path: "voice", element: <Navigate to="../narration" replace /> },
          { path: "narration", lazy: page(() => import("@/pages/settings/studio/NarrationPage")) },
          { path: "pronunciation", lazy: page(() => import("@/pages/settings/studio/PronunciationPage")) },
          { path: "word-timing", lazy: page(() => import("@/pages/settings/studio/WordTimingPage")) },
          // Legacy paths that restored tabs and Recent entries may carry.
          { path: "ai", element: <Navigate to="../agents" replace /> },
          { path: "library", element: <Navigate to="../parsing" replace /> },
          { path: "providers", element: <Navigate to="../opencode" replace /> },
        ],
      },
      // Legacy path with no subject.
      { path: "lectures", element: <Navigate to="/subjects" replace /> },
      // A saved tab or Recent entry can point at a route the app doesn't have.
      { path: "*", element: <Navigate to="/" replace /> },
    ],
  },
];

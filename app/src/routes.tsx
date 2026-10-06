import type { ComponentType } from "react";
import { Navigate, Outlet, type RouteObject } from "react-router-dom";
import { LoadingFill } from "@/components/ui/PageParts";
import { RouteError } from "@/components/ErrorBoundary";
import SubjectLayout from "@/layouts/SubjectLayout";
import SettingsLayout from "@/layouts/SettingsLayout";
import HomePage from "@/pages/HomePage";
import NewTabPage from "@/pages/NewTabPage";
import SubjectsRedirect from "@/pages/SubjectsRedirect";

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
      { path: "chat", lazy: page(() => import("@/pages/ChatPage")) },
      { path: "calendar", lazy: page(() => import("@/pages/CalendarPage")) },
      { path: "projects", lazy: page(() => import("@/pages/ProjectsIndexPage")) },
      { path: "projects/:projectId", lazy: page(() => import("@/pages/ProjectPage")) },
      // Nested under the project, whose columns define its statuses.
      { path: "projects/:projectId/tasks/:taskId", lazy: page(() => import("@/pages/TaskPage")) },
      { path: "tasks", lazy: page(() => import("@/pages/TasksPage")) },
      // An unfiled task: same page, with a null project and the default board.
      { path: "tasks/:taskId", lazy: page(() => import("@/pages/TaskPage")) },
      // No page: reopens the last subject (SubjectsRedirect).
      { path: "subjects", element: <SubjectsRedirect /> },
      // A file or lecture as a whole page (a tab, or the side panel), outside
      // SubjectLayout's tabs.
      { path: "subjects/:subjectId/file", lazy: page(() => import("@/pages/subject/FilePage")) },
      { path: "subjects/:subjectId/lecture", lazy: page(() => import("@/pages/subject/LecturePage")) },
      {
        // SubjectLayout resolves the subject once, via outlet context.
        path: "subjects/:subjectId",
        element: <SubjectLayout />,
        children: [
          { index: true, lazy: page(() => import("@/pages/subject/OverviewPage")) },
          { path: "modules", lazy: page(() => import("@/pages/subject/ModulesPage")) },
          {
            // Sub-tabs are child routes so restore, crumbs and ⌘-click key
            // off the path (see FilesPage).
            path: "files",
            lazy: page(() => import("@/pages/subject/FilesPage")),
            children: [
              { index: true, element: <Navigate to="downloads" replace /> },
              { path: "downloads", lazy: page(() => import("@/pages/subject/DownloadsPage")) },
              { path: "uploads", lazy: page(() => import("@/pages/subject/UploadsPage")) },
              { path: "documents", lazy: page(() => import("@/pages/subject/DocumentsPage")) },
            ],
          },
          { path: "lectures", lazy: page(() => import("@/pages/subject/LecturesPage")) },
          { path: "announcements", lazy: page(() => import("@/pages/subject/AnnouncementsPage")) },
          { path: "assignments", lazy: page(() => import("@/pages/subject/AssignmentsPage")) },
          { path: "discussion", lazy: page(() => import("@/pages/subject/DiscussionPage")) },
          { path: "projects", lazy: page(() => import("@/pages/subject/ProjectsPage")) },
          // Legacy paths that restored tabs and Recent entries may carry.
          { path: "downloads", element: <Navigate to="../files/downloads" replace /> },
          { path: "uploads", element: <Navigate to="../files/uploads" replace /> },
        ],
      },
      { path: "sync", lazy: page(() => import("@/pages/SyncPage")) },
      // The id names a native WebView Rust parks over this content area.
      { path: "browse/:id", lazy: page(() => import("@/pages/BrowserPage")) },
      {
        path: "settings",
        element: <SettingsLayout />,
        children: [
          { index: true, element: <Navigate to="canvas" replace /> },
          { path: "canvas", lazy: page(() => import("@/pages/settings/CanvasPage")) },
          { path: "appearance", lazy: page(() => import("@/pages/settings/AppearancePage")) },
          { path: "browser", lazy: page(() => import("@/pages/settings/BrowserPage")) },
          { path: "storage", lazy: page(() => import("@/pages/settings/StoragePage")) },
          { path: "agents", lazy: page(() => import("@/pages/settings/AgentsPage")) },
          { path: "opencode", lazy: page(() => import("@/pages/settings/OpencodePage")) },
          { path: "jobs", lazy: page(() => import("@/pages/settings/JobsPage")) },
          { path: "parsing", lazy: page(() => import("@/pages/settings/ParsingPage")) },
          { path: "embeddings", lazy: page(() => import("@/pages/settings/EmbeddingsPage")) },
          // Legacy paths that restored tabs and Recent entries may carry.
          { path: "ai", element: <Navigate to="../agents" replace /> },
          { path: "library", element: <Navigate to="../parsing" replace /> },
        ],
          { path: "providers", element: <Navigate to="../opencode" replace /> },
      },
      // Legacy path with no subject.
      { path: "lectures", element: <Navigate to="/subjects" replace /> },
    ],
  },
];

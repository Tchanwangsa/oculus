import { Navigate, Outlet, type RouteObject } from "react-router-dom";
import { RouteError } from "@/components/ErrorBoundary";
import SubjectLayout from "@/layouts/SubjectLayout";
import HomePage from "@/pages/HomePage";
import NewTabPage from "@/pages/NewTabPage";
import ChatPage from "@/pages/ChatPage";
import CalendarPage from "@/pages/CalendarPage";
import ProjectsIndexPage from "@/pages/ProjectsIndexPage";
import ProjectPage from "@/pages/ProjectPage";
import TaskPage from "@/pages/TaskPage";
import TasksPage from "@/pages/TasksPage";
import SubjectsIndexPage from "@/pages/SubjectsIndexPage";
import SubjectOverviewPage from "@/pages/subject/OverviewPage";
import SubjectModulesPage from "@/pages/subject/ModulesPage";
import SubjectFilesPage from "@/pages/subject/FilesPage";
import SubjectDownloadsPage from "@/pages/subject/DownloadsPage";
import SubjectUploadsPage from "@/pages/subject/UploadsPage";
import SubjectDocumentsPage from "@/pages/subject/DocumentsPage";
import SubjectLecturesPage from "@/pages/subject/LecturesPage";
import SubjectAnnouncementsPage from "@/pages/subject/AnnouncementsPage";
import SubjectAssignmentsPage from "@/pages/subject/AssignmentsPage";
import SubjectDiscussionPage from "@/pages/subject/DiscussionPage";
import SubjectProjectsPage from "@/pages/subject/ProjectsPage";
import SubjectFilePage from "@/pages/subject/FilePage";
import SubjectLecturePage from "@/pages/subject/LecturePage";
import SyncPage from "@/pages/SyncPage";
import BrowserPage from "@/pages/BrowserPage";
import SettingsLayout from "@/layouts/SettingsLayout";
import SettingsCanvasPage from "@/pages/settings/CanvasPage";
import SettingsAiPage from "@/pages/settings/AiPage";
import SettingsStoragePage from "@/pages/settings/StoragePage";
import SettingsLibraryPage from "@/pages/settings/LibraryPage";
import SettingsBrowserPage from "@/pages/settings/BrowserPage";
import SettingsAppearancePage from "@/pages/settings/AppearancePage";

/** The route table; each pane builds its own memory router over it
 *  (`app/src/components/tabs/TabPane.tsx`). The shell sits outside them all. */
function PaneRoot() {
  return <Outlet />;
}

export const routes: RouteObject[] = [
  {
    path: "/",
    element: <PaneRoot />,
    errorElement: <RouteError />,
    children: [
      { index: true, element: <HomePage /> },
      // Where the + button and ⌘T land.
      { path: "new", element: <NewTabPage /> },
      { path: "chat", element: <ChatPage /> },
      { path: "calendar", element: <CalendarPage /> },
      { path: "projects", element: <ProjectsIndexPage /> },
      { path: "projects/:projectId", element: <ProjectPage /> },
      // Nested under the project, whose columns define its statuses.
      { path: "projects/:projectId/tasks/:taskId", element: <TaskPage /> },
      { path: "tasks", element: <TasksPage /> },
      // An unfiled task: same page, with a null project and the default board.
      { path: "tasks/:taskId", element: <TaskPage /> },
      { path: "subjects", element: <SubjectsIndexPage /> },
      // A peek expanded to a full page, outside SubjectLayout's tabs.
      { path: "subjects/:subjectId/file", element: <SubjectFilePage /> },
      { path: "subjects/:subjectId/lecture", element: <SubjectLecturePage /> },
      {
        // SubjectLayout resolves the subject once, via outlet context.
        path: "subjects/:subjectId",
        element: <SubjectLayout />,
        children: [
          { index: true, element: <SubjectOverviewPage /> },
          { path: "modules", element: <SubjectModulesPage /> },
          {
            // Sub-tabs are child routes so restore, crumbs and ⌘-click key
            // off the path (see FilesPage).
            path: "files",
            element: <SubjectFilesPage />,
            children: [
              { index: true, element: <Navigate to="downloads" replace /> },
              { path: "downloads", element: <SubjectDownloadsPage /> },
              { path: "uploads", element: <SubjectUploadsPage /> },
              { path: "documents", element: <SubjectDocumentsPage /> },
            ],
          },
          { path: "lectures", element: <SubjectLecturesPage /> },
          { path: "announcements", element: <SubjectAnnouncementsPage /> },
          { path: "assignments", element: <SubjectAssignmentsPage /> },
          { path: "discussion", element: <SubjectDiscussionPage /> },
          { path: "projects", element: <SubjectProjectsPage /> },
          // Legacy paths that restored tabs and Recent entries may carry.
          { path: "downloads", element: <Navigate to="../files/downloads" replace /> },
          { path: "uploads", element: <Navigate to="../files/uploads" replace /> },
        ],
      },
      { path: "sync", element: <SyncPage /> },
      // The id names a native WebView Rust parks over this content area.
      { path: "browse/:id", element: <BrowserPage /> },
      {
        path: "settings",
        element: <SettingsLayout />,
        children: [
          { index: true, element: <Navigate to="canvas" replace /> },
          { path: "canvas", element: <SettingsCanvasPage /> },
          { path: "ai", element: <SettingsAiPage /> },
          { path: "storage", element: <SettingsStoragePage /> },
          { path: "library", element: <SettingsLibraryPage /> },
          { path: "browser", element: <SettingsBrowserPage /> },
          { path: "appearance", element: <SettingsAppearancePage /> },
        ],
      },
      // Legacy path with no subject.
      { path: "lectures", element: <Navigate to="/subjects" replace /> },
    ],
  },
];

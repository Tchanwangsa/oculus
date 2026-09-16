import { Navigate, Outlet, type RouteObject } from "react-router-dom";
import SubjectLayout from "@/layouts/SubjectLayout";
import HomePage from "@/pages/HomePage";
import ChatPage from "@/pages/ChatPage";
import CalendarPage from "@/pages/CalendarPage";
import ProjectsIndexPage from "@/pages/ProjectsIndexPage";
import ProjectPage from "@/pages/ProjectPage";
import TaskPage from "@/pages/TaskPage";
import SubjectsIndexPage from "@/pages/SubjectsIndexPage";
import SubjectOverviewPage from "@/pages/subject/OverviewPage";
import SubjectModulesPage from "@/pages/subject/ModulesPage";
import SubjectDownloadsPage from "@/pages/subject/DownloadsPage";
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

/**
 * The route table, shared by every tab. Each tab builds its own memory router
 * over it (`app/src/components/tabs/TabPane.tsx`), so this is a plain table
 * rather than a router: the shell — sidebar, tab strip, palette — lives
 * *outside* all of them and is no longer a route element.
 */

/** The root of a tab's tree. It is the pane itself, so it renders nothing of
 *  its own; the shell that used to sit here is now above every router. */
function PaneRoot() {
  return <Outlet />;
}

export const routes: RouteObject[] = [
  {
    path: "/",
    element: <PaneRoot />,
    children: [
      { index: true, element: <HomePage /> },
      { path: "chat", element: <ChatPage /> },
      { path: "calendar", element: <CalendarPage /> },
      { path: "projects", element: <ProjectsIndexPage /> },
      // One project, with its board / table / backlog views inside it.
      { path: "projects/:projectId", element: <ProjectPage /> },
      // One task as a page of its own — a description, its metadata and its
      // subtasks. Nested under the project because the page needs the
      // project's columns to say what a status is.
      { path: "projects/:projectId/tasks/:taskId", element: <TaskPage /> },
      { path: "subjects", element: <SubjectsIndexPage /> },
      // A file/lecture promoted to a full page (peek → expand). Outside
      // SubjectLayout: full pages take the whole content area, Notion-style.
      { path: "subjects/:subjectId/file", element: <SubjectFilePage /> },
      { path: "subjects/:subjectId/lecture", element: <SubjectLecturePage /> },
      {
        // Everything for one subject lives under its id; SubjectLayout resolves
        // it once and hands it to the tabs via outlet context.
        path: "subjects/:subjectId",
        element: <SubjectLayout />,
        children: [
          { index: true, element: <SubjectOverviewPage /> },
          { path: "modules", element: <SubjectModulesPage /> },
          { path: "downloads", element: <SubjectDownloadsPage /> },
          { path: "lectures", element: <SubjectLecturesPage /> },
          { path: "announcements", element: <SubjectAnnouncementsPage /> },
          { path: "assignments", element: <SubjectAssignmentsPage /> },
          { path: "discussion", element: <SubjectDiscussionPage /> },
          { path: "projects", element: <SubjectProjectsPage /> },
          // The old Files tab is gone; its bookmarks land on Downloads.
          { path: "files", element: <Navigate to="../downloads" replace /> },
        ],
      },
      { path: "sync", element: <SyncPage /> },
      // An in-app browser tab: the id names a native page WebView that Rust
      // parks over this route's content area. See BrowserPage.
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
        ],
      },
      // Old top-level /lectures had no subject — send it to the picker.
      { path: "lectures", element: <Navigate to="/subjects" replace /> },
    ],
  },
];

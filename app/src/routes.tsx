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
import SettingsBrowserPage from "@/pages/settings/BrowserPage";

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
    // Caught here rather than per page: every route below is a pane's whole
    // content area, so this is the smallest thing worth losing.
    errorElement: <RouteError />,
    children: [
      { index: true, element: <HomePage /> },
      // Where the + button and ⌘T land. Not Home: a tab you opened to put
      // something beside what you are reading does not want a dashboard.
      { path: "new", element: <NewTabPage /> },
      { path: "chat", element: <ChatPage /> },
      { path: "calendar", element: <CalendarPage /> },
      { path: "projects", element: <ProjectsIndexPage /> },
      // One project, with its board / table / backlog views inside it.
      { path: "projects/:projectId", element: <ProjectPage /> },
      // One task as a page of its own — a description, its metadata and its
      // subtasks. Nested under the project because the page needs the
      // project's columns to say what a status is.
      { path: "projects/:projectId/tasks/:taskId", element: <TaskPage /> },
      // Every task across every project, plus the ones filed nowhere at all.
      { path: "tasks", element: <TasksPage /> },
      // An unfiled task's own page. The same component as the filed route
      // above, which reads its project as `null` and its board as the default
      // one — there is no project segment to nest it under, and inventing an
      // "Inbox" project to have one was the decision this route replaces.
      { path: "tasks/:taskId", element: <TaskPage /> },
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
          { path: "browser", element: <SettingsBrowserPage /> },
        ],
      },
      // Old top-level /lectures had no subject — send it to the picker.
      { path: "lectures", element: <Navigate to="/subjects" replace /> },
    ],
  },
];

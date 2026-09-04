import { createHashRouter, RouterProvider, Navigate } from "react-router-dom";
import { useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { applyTheme, getStoredTheme } from "@/lib/theme";
import { getDb, getParseSettings, reconcileStaleSyncRuns } from "@/lib/db";
import { useBackendEvents } from "@/hooks/useBackendEvents";
import { useQualitySweep } from "@/hooks/useQualitySweep";
import { useAutomations } from "@/hooks/useAutomations";
import { watchNewFiles } from "@/stores/newFilesStore";
import { watchLectureDownloads } from "@/stores/lectureDownloadStore";
import AppLayout from "@/layouts/AppLayout";
import SubjectLayout from "@/layouts/SubjectLayout";
import ChatPage from "@/pages/ChatPage";
import CalendarPage from "@/pages/CalendarPage";
import SubjectsIndexPage from "@/pages/SubjectsIndexPage";
import SubjectOverviewPage from "@/pages/subject/OverviewPage";
import SubjectModulesPage from "@/pages/subject/ModulesPage";
import SubjectDownloadsPage from "@/pages/subject/DownloadsPage";
import SubjectLecturesPage from "@/pages/subject/LecturesPage";
import SubjectAnnouncementsPage from "@/pages/subject/AnnouncementsPage";
import SubjectAssignmentsPage from "@/pages/subject/AssignmentsPage";
import SubjectDiscussionPage from "@/pages/subject/DiscussionPage";
import SubjectFilePage from "@/pages/subject/FilePage";
import SubjectLecturePage from "@/pages/subject/LecturePage";
import SyncPage from "@/pages/SyncPage";
import AutomationsPage from "@/pages/AutomationsPage";
import AutomationEditorPage from "@/pages/AutomationEditorPage";
import InboxPage from "@/pages/InboxPage";
import SettingsLayout from "@/layouts/SettingsLayout";
import SettingsCanvasPage from "@/pages/settings/CanvasPage";
import SettingsAiPage from "@/pages/settings/AiPage";
import SettingsStoragePage from "@/pages/settings/StoragePage";
import SettingsLibraryPage from "@/pages/settings/LibraryPage";

const router = createHashRouter([
  {
    path: "/",
    element: <AppLayout />,
    children: [
      { index: true, element: <Navigate to="/chat" replace /> },
      { path: "chat", element: <ChatPage /> },
      { path: "calendar", element: <CalendarPage /> },
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
          // The old Files tab is gone; its bookmarks land on Downloads.
          { path: "files", element: <Navigate to="../downloads" replace /> },
        ],
      },
      { path: "sync", element: <SyncPage /> },
      { path: "inbox", element: <InboxPage /> },
      { path: "automations", element: <AutomationsPage /> },
      { path: "automations/:id", element: <AutomationEditorPage /> },
      // Scheduled Tasks became Automations; its rows migrated with it.
      { path: "schedules", element: <Navigate to="/automations" replace /> },
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
]);

function EventBridge() {
  useBackendEvents();
  useQualitySweep();
  useAutomations();
  useEffect(() => watchNewFiles(), []);
  useEffect(() => watchLectureDownloads(), []);
  return null;
}

export default function App() {
  useEffect(() => {
    applyTheme(getStoredTheme());

    // tauri-plugin-sql runs migrations on first load, not at app startup, so
    // the schema only existed once you happened to open a page that queried
    // it. The app opens on /chat, which reads the index through a Rust command
    // and never touched the plugin — leaving `pages` missing. Load it here so
    // the schema is up to date before any page mounts.
    getDb()
      .then(async () => {
        const [n, parse] = await Promise.all([
          reconcileStaleSyncRuns(),
          getParseSettings(),
        ]);
        if (n) console.warn(`marked ${n} interrupted sync run(s) failed`);
        await invoke("sidecar_set_limits", {
          memoryCapMb: parse.memoryCapMb,
          backend: parse.backend,
        }).catch(() => {});
      })
      .catch((e) => console.error("db init failed", e));

    const mediaQuery = window.matchMedia("(prefers-color-scheme: dark)");
    const handleChange = () => {
      if (getStoredTheme() === "system") applyTheme("system");
    };
    mediaQuery.addEventListener("change", handleChange);
    return () => mediaQuery.removeEventListener("change", handleChange);
  }, []);

  return (
    <>
      <EventBridge />
      <RouterProvider router={router} />
    </>
  );
}

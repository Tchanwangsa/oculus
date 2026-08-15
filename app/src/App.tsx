import { createHashRouter, RouterProvider, Navigate } from "react-router-dom";
import { useEffect } from "react";
import { applyTheme, getStoredTheme } from "@/lib/theme";
import { getDb, reconcileStaleSyncRuns } from "@/lib/db";
import { useBackendEvents } from "@/hooks/useBackendEvents";
import AppLayout from "@/layouts/AppLayout";
import ChatPage from "@/pages/ChatPage";
import LecturesPage from "@/pages/LecturesPage";
import SubjectsPage from "@/pages/SubjectsPage";
import SyncPage from "@/pages/SyncPage";

const router = createHashRouter([
  {
    path: "/",
    element: <AppLayout />,
    children: [
      { index: true, element: <Navigate to="/chat" replace /> },
      { path: "chat",     element: <ChatPage /> },
      { path: "lectures", element: <LecturesPage /> },
      { path: "subjects", element: <SubjectsPage /> },
      { path: "sync",     element: <SyncPage /> },
    ],
  },
]);

function EventBridge() {
  useBackendEvents();
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
      .then(reconcileStaleSyncRuns)
      .then((n) => n && console.warn(`marked ${n} interrupted sync run(s) failed`))
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

import { createHashRouter, RouterProvider, Navigate } from "react-router-dom";
import { useEffect } from "react";
import { applyTheme, getStoredTheme } from "@/lib/theme";
import AppLayout from "@/layouts/AppLayout";
import ChatPage from "@/pages/ChatPage";
import LecturesPage from "@/pages/LecturesPage";
import GraphPage from "@/pages/GraphPage";
import SubjectsPage from "@/pages/SubjectsPage";
import NotificationsPage from "@/pages/NotificationsPage";
import SyncPage from "@/pages/SyncPage";

const router = createHashRouter([
  {
    path: "/",
    element: <AppLayout />,
    children: [
      { index: true, element: <Navigate to="/chat" replace /> },
      { path: "chat",          element: <ChatPage /> },
      { path: "lectures",      element: <LecturesPage /> },
      { path: "graph",         element: <GraphPage /> },
      { path: "subjects",      element: <SubjectsPage /> },
      { path: "notifications", element: <NotificationsPage /> },
      { path: "sync",          element: <SyncPage /> },
    ],
  },
]);

export default function App() {
  useEffect(() => {
    applyTheme(getStoredTheme());

    const mediaQuery = window.matchMedia("(prefers-color-scheme: dark)");
    const handleChange = () => {
      if (getStoredTheme() === "system") applyTheme("system");
    };
    mediaQuery.addEventListener("change", handleChange);
    return () => mediaQuery.removeEventListener("change", handleChange);
  }, []);

  return <RouterProvider router={router} />;
}

import { useEffect } from "react";
import { applyTheme, getStoredTheme } from "@/lib/theme";
import { getDb, reconcileStaleSyncRuns } from "@/lib/db";
import { useBackendEvents } from "@/hooks/useBackendEvents";
import { useQualitySweep } from "@/hooks/useQualitySweep";
import { watchNewFiles } from "@/stores/newFilesStore";
import { watchLectureDownloads } from "@/stores/lectureDownloadStore";
import AppLayout from "@/layouts/AppLayout";
import { ErrorBoundary } from "@/components/ErrorBoundary";

function EventBridge() {
  useBackendEvents();
  useQualitySweep();
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
        const n = await reconcileStaleSyncRuns();
        if (n) console.warn(`marked ${n} interrupted sync run(s) failed`);
      })
      .catch((e) => console.error("db init failed", e));

    const mediaQuery = window.matchMedia("(prefers-color-scheme: dark)");
    const handleChange = () => {
      if (getStoredTheme() === "system") applyTheme("system");
    };
    mediaQuery.addEventListener("change", handleChange);
    return () => mediaQuery.removeEventListener("change", handleChange);
  }, []);

  // The shell is no longer a route element: it is above every tab's router
  // (`app/src/routes.tsx`), and the tabs are mounted inside it.
  return (
    <ErrorBoundary>
      <EventBridge />
      <AppLayout />
    </ErrorBoundary>
  );
}

import { useEffect } from "react";
import { applyTheme, getStoredTheme, watchSystemTheme } from "@/lib/ui/theme";
import { getDb, reconcileStaleSyncRuns } from "@/lib/db";
import { useBackendEvents } from "@/hooks/backend/useBackendEvents";
import { useQualitySweep } from "@/hooks/sync/useQualitySweep";
import { watchNewFiles } from "@/stores/sync/newFilesStore";
import { watchLectureDownloads } from "@/stores/lectures/lectureDownloadStore";
import { restoreIndexQueue } from "@/stores/sync/indexStore";
import AppLayout from "@/layouts/AppLayout";
import { ErrorBoundary } from "@/components/ErrorBoundary";

function EventBridge() {
  useBackendEvents();
  useQualitySweep();
  useEffect(() => watchNewFiles(), []);
  useEffect(() => watchLectureDownloads(), []);
  // Boot only: Settings re-checks the key but must never restart metered work.
  useEffect(() => void restoreIndexQueue(), []);
  return null;
}

export default function App() {
  useEffect(() => {
    applyTheme(getStoredTheme());

    // tauri-plugin-sql migrates on first load, and a page may never query it
    // (chat goes through Rust), so load it before any page mounts.
    getDb()
      .then(async () => {
        const n = await reconcileStaleSyncRuns();
        if (n) console.warn(`marked ${n} interrupted sync run(s) failed`);
      })
      .catch((e) => console.error("db init failed", e));

    return watchSystemTheme();
  }, []);

  return (
    <ErrorBoundary>
      <EventBridge />
      <AppLayout />
    </ErrorBoundary>
  );
}

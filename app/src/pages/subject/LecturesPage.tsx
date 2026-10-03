import { LectureRow } from "@/components/lectures/LectureRow";
import { SubjectPage } from "@/components/subjects/SubjectPage";
import { useCallback, useEffect, useState } from "react";
import {
  ArrowsClockwise,
  Play,
  WarningCircle,
} from "@phosphor-icons/react";
import { invoke } from "@tauri-apps/api/core";
import {
  LECTURE_DOWNLOADED_EVENT,
  deleteLectureVideo,
  downloadLecture,
  wasCancelled,
} from "@/stores/lectureDownloadStore";
import { ConfirmDialog } from "@/components/ui/ConfirmDialog";
import { ListCard } from "@/components/ui/PageParts";
import { useWindowEvent } from "@/hooks/useEvents";
import { Button } from "@/components/ui/button";
import { Alert, AlertDescription } from "@/components/ui/alert";
import {
  getLectures,
  upsertLectures,
  type Lecture,
  type LectureData,
} from "@/lib/db";
import { useSubject } from "@/layouts/SubjectLayout";
import { useSidePanelStore } from "@/stores/sidePanelStore";
import { useTabId } from "@/components/tabs/TabContext";
import { recordRecent } from "@/lib/recents";
import {
  LECTURES_CHANGED_EVENT,
} from "@/lib/lectures";

/** The subject's lectures. Selecting one opens the player in the side panel. */
export default function SubjectLecturesPage() {
  const subject = useSubject();
  const [lectures, setLectures] = useState<Lecture[]>([]);
  // What is open lives in the panel, so closing it un-highlights the row.
  const tabId = useTabId();
  const openInPanel = useSidePanelStore((s) => s.open);
  const syncPanel = useSidePanelStore((s) => s.sync);
  const openItem = useSidePanelStore((s) => s.items[tabId]);
  const selectedId = openItem?.kind === "lecture" ? openItem.lecture.id : null;
  const [syncing, setSyncing] = useState(false);
  const [syncError, setSyncError] = useState<string | null>(null);
  const [pendingDelete, setPendingDelete] = useState<Lecture | null>(null);

  const refreshLectures = useCallback(async () => {
    const rows = await getLectures(subject.id);
    setLectures(rows);
    // Hand the panel the fresh row for whatever it has open.
    const open = useSidePanelStore.getState().items[tabId];
    if (open?.kind === "lecture") {
      const fresh = rows.find((r) => r.id === open.lecture.id);
      if (fresh) syncPanel(tabId, { kind: "lecture", lecture: fresh });
    }
  }, [subject.id, tabId, syncPanel]);

  useEffect(() => {
    refreshLectures();
  }, [refreshLectures]);

  // The panel's player has no list to call back into, so it says so here; a
  // download fires once the finished file is in the DB.
  useWindowEvent([LECTURES_CHANGED_EVENT, LECTURE_DOWNLOADED_EVENT], () => refreshLectures());

  const handleDownload = useCallback((lec: Lecture) => {
    setSyncError(null);
    downloadLecture(lec).catch((e) => {
      if (!wasCancelled(e)) setSyncError(`Download failed: ${e}`);
    });
  }, []);

  const handleDelete = async (lec: Lecture) => {
    setPendingDelete(null);
    setSyncError(null);
    try {
      await deleteLectureVideo(lec.id);
      await refreshLectures();
    } catch (e) {
      setSyncError(`Could not delete the download: ${e}`);
    }
  };

  const handleSync = async () => {
    setSyncing(true);
    setSyncError(null);
    try {
      const data = await invoke<LectureData[]>("echo360_sync_lectures", {
        canvasCourseId: subject.id,
      });
      await upsertLectures(subject.id, data);
      await refreshLectures();
    } catch (e) {
      setSyncError(String(e));
    } finally {
      setSyncing(false);
    }
  };

  const handleSelectLecture = useCallback((lec: Lecture) => {
    openInPanel({ kind: "lecture", lecture: lec });
    recordRecent(subject.id, { kind: "lecture", ref: lec.id, title: lec.title });
  }, [openInPanel, subject.id]);

  return (
    <>
      <SubjectPage>
        <div className="mb-3 flex items-center justify-end">
          <Button
            variant="ghost"
            size="sm"
            className="h-6 gap-1.5 px-2 text-[11px] text-muted-foreground"
            onClick={handleSync}
            disabled={syncing}
          >
            <ArrowsClockwise size={11} className={syncing ? "animate-spin" : ""} />
            {syncing ? "Syncing…" : "Sync lectures"}
          </Button>
        </div>

        {syncError && (
          <Alert variant="destructive" className="mb-3 w-auto px-2.5 py-2">
            <WarningCircle />
            <AlertDescription className="text-[11px] break-words">
              {syncError}
            </AlertDescription>
          </Alert>
        )}

        {lectures.length === 0 ? (
          <div className="flex flex-col items-center justify-center py-16 gap-2 text-center">
            <Play size={24} className="text-muted-foreground/40" />
            <p className="text-sm text-muted-foreground">No lectures synced yet.</p>
          </div>
        ) : (
          <ListCard>
            {lectures.map((lec) => (
              <LectureRow
                key={lec.id}
                lecture={lec}
                active={selectedId === lec.id}
                onSelect={handleSelectLecture}
                onDownload={handleDownload}
                onDelete={setPendingDelete}
              />
            ))}
          </ListCard>
        )}
      </SubjectPage>

      <ConfirmDialog
        open={!!pendingDelete}
        onCancel={() => setPendingDelete(null)}
        title="Delete this download?"
        description={
          pendingDelete
            ? `The video file for “${pendingDelete.title}” is removed from this Mac. Your place in it, the transcript and any chapters or reading copy are kept, and you can download it again whenever you want.`
            : ""
        }
        confirmLabel="Delete"
        onConfirm={() => pendingDelete && handleDelete(pendingDelete)}
      />
    </>
  );
}

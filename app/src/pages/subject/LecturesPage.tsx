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
  setLectureDone,
  upsertLectures,
  type Lecture,
  type LectureData,
} from "@/lib/db";
import { useSubject } from "@/layouts/SubjectLayout";
import { sideFront, useTabStore } from "@/stores/tabStore";
import { usePaneTab } from "@/components/tabs/TabContext";
import { openBeside } from "@/lib/tabRouters";
import { LECTURE_PROGRESS_EVENT } from "@/lib/lecturePlayback";
import { findStartedLectureEnds, isWatched } from "@/lib/lectureEnd";
import { recordRecent } from "@/lib/recents";
import {
  LECTURES_CHANGED_EVENT,
  lecturePageId,
  lecturePagePath,
} from "@/lib/lectures";

/** The subject's lectures. Selecting one opens its page in the side panel. */
export default function SubjectLecturesPage() {
  const subject = useSubject();
  const [lectures, setLectures] = useState<Lecture[]>([]);
  // The row lit is the lecture in front of this tab's side panel, so sending
  // it to the back or closing the panel un-highlights it.
  const { tabId } = usePaneTab();
  const selectedId = useTabStore((s) => {
    const tab = s.tabs.find((t) => t.id === tabId);
    return lecturePageId(tab && sideFront(tab)?.path);
  });
  const [syncing, setSyncing] = useState(false);
  const [syncError, setSyncError] = useState<string | null>(null);
  const [pendingDelete, setPendingDelete] = useState<Lecture | null>(null);

  const refreshLectures = useCallback(async () => {
    const rows = await getLectures(subject.id);
    setLectures(rows);
    findStartedLectureEnds(rows);
  }, [subject.id]);

  useEffect(() => {
    refreshLectures();
  }, [refreshLectures]);

  // Re-read on any lecture write: a player beside the list (the side panel)
  // saving progress, a download once the finished file is in the DB.
  useWindowEvent(
    [LECTURES_CHANGED_EVENT, LECTURE_DOWNLOADED_EVENT, LECTURE_PROGRESS_EVENT],
    () => refreshLectures(),
  );

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

  const handleToggleDone = useCallback(async (lec: Lecture) => {
    setSyncError(null);
    try {
      const rewind = !!lec.completed && isWatched(lec, lec.progress_seconds, 0);
      await setLectureDone(lec.id, !lec.completed, rewind);
      window.dispatchEvent(new CustomEvent(LECTURES_CHANGED_EVENT));
    } catch (e) {
      setSyncError(`Could not update the lecture: ${e}`);
    }
  }, []);

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
    openBeside(lecturePagePath(lec));
    recordRecent(subject.id, { kind: "lecture", ref: lec.id, title: lec.title });
  }, [subject.id]);

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
                onToggleDone={handleToggleDone}
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

import { SubjectPage } from "@/components/subjects/SubjectPage";
import { useCallback, useEffect, useState } from "react";
import {
  ArrowsClockwise,
  CheckCircle,
  CircleNotch,
  Clock,
  DownloadSimple,
  Play,
  Trash,
  WarningCircle,
  X,
} from "@phosphor-icons/react";
import { invoke } from "@tauri-apps/api/core";
import { cn } from "@/lib/utils";
import {
  LECTURE_DOWNLOADED_EVENT,
  cancelLectureDownload,
  deleteLectureVideo,
  downloadLecture,
  isDownloading,
  useLectureDownloads,
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
  fmtDurationSecs,
  fmtLectureDate,
  lecturePagePath,
  progressLabel,
  LECTURES_CHANGED_EVENT,
} from "@/lib/lectures";

/**
 * A control inside the row's own `button`: a `div` with a button role, because
 * a nested `<button>` is invalid HTML and WebKit swallows its clicks. Stops
 * propagation so it never also opens the lecture.
 */
function RowAction({
  label,
  onTrigger,
  className,
  children,
}: {
  label: string;
  onTrigger: () => void;
  className?: string;
  children: React.ReactNode;
}) {
  return (
    <div
      role="button"
      tabIndex={0}
      data-tab-skip
      aria-label={label}
      title={label}
      onClick={(e) => {
        e.stopPropagation();
        onTrigger();
      }}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          e.stopPropagation();
          onTrigger();
        }
      }}
      className={cn(
        "-m-1 p-1 rounded text-muted-foreground/50 transition-colors",
        "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50",
        className,
      )}
    >
      {children}
    </div>
  );
}

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
  const downloads = useLectureDownloads();

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

  const handleDownload = (lec: Lecture) => {
    setSyncError(null);
    downloadLecture(lec).catch((e) => {
      if (!wasCancelled(e)) setSyncError(`Download failed: ${e}`);
    });
  };

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

  const handleSelectLecture = (lec: Lecture) => {
    openInPanel({ kind: "lecture", lecture: lec });
    recordRecent(subject.id, { kind: "lecture", ref: lec.id, title: lec.title });
  };

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
            {lectures.map((lec) => {
              const active = selectedId === lec.id;
              const pl = progressLabel(lec);
              const prog = downloads.progress[lec.id];
              const isDown = !lec.video_path && isDownloading(downloads, lec.id);
              return (
                <button
                  key={lec.id}
                  /* ⌘-click opens the lecture as a page of its own. */
                  data-tab-href={lecturePagePath(lec)}
                  onClick={() => handleSelectLecture(lec)}
                  className={cn(
                    "w-full text-left px-3 py-2.5 flex gap-3 items-center hover:bg-surface transition-colors",
                    active && "bg-surface-raised",
                  )}
                >
                  <div className="shrink-0">
                    {lec.completed ? (
                      <CheckCircle size={14} className="text-success" />
                    ) : (
                      <div
                        className={cn(
                          "w-3 h-3 rounded-full border-2",
                          active ? "border-brand" : "border-muted-foreground/40",
                        )}
                      />
                    )}
                  </div>
                  <div className="min-w-0 flex-1">
                    <p className="text-[12px] font-medium text-foreground truncate leading-tight">
                      {lec.title}
                    </p>
                  </div>
                  <span className="shrink-0 text-[11px] text-muted-foreground">
                    {fmtLectureDate(lec.date)}
                  </span>
                  <span className="shrink-0 text-[11px] text-muted-foreground flex items-center gap-1 w-18 whitespace-nowrap">
                    <Clock size={10} />
                    {fmtDurationSecs(lec.duration_seconds)}
                  </span>
                  <span className={cn("shrink-0 text-[11px] w-20 text-right", pl.color)}>
                    {pl.text}
                  </span>
                  {/* Downloaded ✓ + delete · downloading NN% + cancel ·
                      otherwise a download trigger. */}
                  <span className="shrink-0 w-16 flex items-center justify-end gap-1.5">
                    {lec.video_path ? (
                      <>
                        <CheckCircle size={11} className="text-success" />
                        <RowAction
                          label="Delete download"
                          onTrigger={() => setPendingDelete(lec)}
                          className="hover:text-destructive"
                        >
                          <Trash size={11} />
                        </RowAction>
                      </>
                    ) : isDown ? (
                      <>
                        {prog == null || prog.phase === "trimming" ? (
                          <CircleNotch size={11} className="animate-spin text-brand" />
                        ) : (
                          <span className="text-[10px] tabular-nums text-brand">
                            {prog.percent}%
                          </span>
                        )}
                        {/* Trimming has no transfer left to cancel. */}
                        {prog?.phase !== "trimming" && (
                          <RowAction
                            label="Cancel download"
                            onTrigger={() => cancelLectureDownload(lec.id)}
                            className="hover:text-destructive"
                          >
                            <X size={11} />
                          </RowAction>
                        )}
                      </>
                    ) : (
                      <RowAction
                        label="Download video"
                        onTrigger={() => handleDownload(lec)}
                        className="hover:text-foreground"
                      >
                        <DownloadSimple size={11} />
                      </RowAction>
                    )}
                  </span>
                </button>
              );
            })}
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

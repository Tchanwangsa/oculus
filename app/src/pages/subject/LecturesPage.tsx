import { useCallback, useEffect, useState } from "react";
import {
  ArrowsClockwise,
  CheckCircle,
  CircleNotch,
  Clock,
  DownloadSimple,
  Play,
  WarningCircle,
} from "@phosphor-icons/react";
import { invoke } from "@tauri-apps/api/core";
import { cn } from "@/lib/utils";
import {
  LECTURE_DOWNLOADED_EVENT,
  downloadLecture,
  isDownloading,
  useLectureDownloads,
} from "@/stores/lectureDownloadStore";
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
  fmtDuration,
  fmtLectureDate,
  lecturePagePath,
  progressLabel,
  LECTURES_CHANGED_EVENT,
} from "@/lib/lectures";

/**
 * Flat list of the subject's lectures. Selecting one opens the player in the
 * side panel; the panel's expand button promotes it to a fully standalone page
 * (`/subjects/:id/lecture`) in its own tab.
 */
export default function SubjectLecturesPage() {
  const subject = useSubject();
  const [lectures, setLectures] = useState<Lecture[]>([]);
  // What is open lives in the panel, not here: the row highlight and the
  // player are then the same fact, and closing the panel un-highlights the row
  // without this page hearing about it.
  const tabId = useTabId();
  const openInPanel = useSidePanelStore((s) => s.open);
  const syncPanel = useSidePanelStore((s) => s.sync);
  const openItem = useSidePanelStore((s) => s.items[tabId]);
  const selectedId = openItem?.kind === "lecture" ? openItem.lecture.id : null;
  const [syncing, setSyncing] = useState(false);
  const [syncError, setSyncError] = useState<string | null>(null);
  const downloads = useLectureDownloads();

  const refreshLectures = useCallback(async () => {
    const rows = await getLectures(subject.id);
    setLectures(rows);
    // Hand the panel the fresh row for whatever it has open, so a progress
    // tick or a finished download reaches the player too.
    const open = useSidePanelStore.getState().items[tabId];
    if (open?.kind === "lecture") {
      const fresh = rows.find((r) => r.id === open.lecture.id);
      if (fresh) syncPanel(tabId, { kind: "lecture", lecture: fresh });
    }
  }, [subject.id, tabId, syncPanel]);

  useEffect(() => {
    refreshLectures();
  }, [refreshLectures]);

  // The panel's player has no list to call back into, so it says so here.
  useEffect(() => {
    const h = () => refreshLectures();
    window.addEventListener(LECTURES_CHANGED_EVENT, h);
    return () => window.removeEventListener(LECTURES_CHANGED_EVENT, h);
  }, [refreshLectures]);

  // A finished download (started here or in a player) lands in the DB before
  // this event fires, so the refetch sees the new paths.
  useEffect(() => {
    const h = () => refreshLectures();
    window.addEventListener(LECTURE_DOWNLOADED_EVENT, h);
    return () => window.removeEventListener(LECTURE_DOWNLOADED_EVENT, h);
  }, [refreshLectures]);

  const handleDownload = (lec: Lecture) => {
    setSyncError(null);
    downloadLecture(lec).catch((e) => setSyncError(`Download failed: ${e}`));
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
    // Feeds the "Recently visited" row on the subject home.
    recordRecent(subject.id, { kind: "lecture", ref: lec.id, title: lec.title });
  };

  return (
    <>
      <div className="page-scroll">
        <div className="mx-auto max-w-5xl px-6 py-5">
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
            <div className="rounded-lg border border-border divide-y divide-border-subtle overflow-hidden">
              {lectures.map((lec) => {
                const active = selectedId === lec.id;
                const pl = progressLabel(lec);
                const prog = downloads.progress[lec.id];
                const isDown = !lec.video_path && isDownloading(downloads, lec.id);
                return (
                  <button
                    key={lec.id}
                    /* The row opens the lecture as a peek; ⌘-click wants the
                       same lecture as a page of its own, which is where this
                       leads (`lib/newTabClicks.ts`). */
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
                      {fmtDuration(lec.duration_seconds)}
                    </span>
                    <span className={cn("shrink-0 text-[11px] w-20 text-right", pl.color)}>
                      {pl.text}
                    </span>
                    {/* Downloaded ✓ · downloading NN% · otherwise a live
                        download trigger — no need to open the player first. */}
                    <span className="shrink-0 w-8 flex items-center justify-end">
                      {lec.video_path ? (
                        <CheckCircle size={11} className="text-success" />
                      ) : isDown ? (
                        prog == null || prog.phase === "trimming" ? (
                          <CircleNotch size={11} className="animate-spin text-brand" />
                        ) : (
                          <span className="text-[10px] tabular-nums text-brand">
                            {prog.percent}%
                          </span>
                        )
                      ) : (
                        <span
                          role="button"
                          tabIndex={0}
                          data-tab-skip
                          aria-label="Download video"
                          title="Download video"
                          onClick={(e) => {
                            e.stopPropagation();
                            handleDownload(lec);
                          }}
                          onKeyDown={(e) => {
                            if (e.key === "Enter" || e.key === " ") {
                              e.preventDefault();
                              e.stopPropagation();
                              handleDownload(lec);
                            }
                          }}
                          className="text-muted-foreground/50 hover:text-foreground transition-colors"
                        >
                          <DownloadSimple size={11} />
                        </span>
                      )}
                    </span>
                  </button>
                );
              })}
            </div>
          )}
        </div>
      </div>
    </>
  );
}

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
import { useNavigate } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import { cn } from "@/lib/utils";
import { useTabStore } from "@/stores/tabStore";
import {
  LECTURE_DOWNLOADED_EVENT,
  downloadLecture,
  isDownloading,
  useLectureDownloads,
} from "@/stores/lectureDownloadStore";
import { Button } from "@/components/ui/button";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { getLectures, upsertLectures, type Lecture } from "@/lib/db";
import { useSubject } from "@/layouts/SubjectLayout";
import { PeekPanel } from "@/components/peek/PeekPanel";
import { LecturePlayer } from "@/components/lectures/LecturePlayer";
import { stopLecturePlayback } from "@/lib/lecturePlayback";
import { recordRecent } from "@/lib/recents";
import {
  fmtDuration,
  fmtLectureDate,
  progressLabel,
  lecturePagePath,
} from "@/lib/lectures";

/**
 * Flat list of the subject's lectures. Selecting one opens the player in a
 * peek; the peek's expand button promotes it to a fully standalone page
 * (`/subjects/:id/lecture`) in its own tab.
 */
export default function SubjectLecturesPage() {
  const subject = useSubject();
  const [lectures, setLectures] = useState<Lecture[]>([]);
  const [selectedLecture, setSelectedLecture] = useState<Lecture | null>(null);
  const [syncing, setSyncing] = useState(false);
  const [syncError, setSyncError] = useState<string | null>(null);
  const downloads = useLectureDownloads();

  const navigate = useNavigate();
  const addTab = useTabStore((s) => s.addTab);

  const refreshLectures = useCallback(async () => {
    const rows = await getLectures(subject.id);
    setLectures(rows);
    // Update selected lecture if still in list
    setSelectedLecture((prev) =>
      prev ? (rows.find((r) => r.id === prev.id) ?? null) : null,
    );
  }, [subject.id]);

  useEffect(() => {
    refreshLectures();
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
      const data = await invoke<
        {
          id: string;
          lesson_id: string;
          title: string;
          date: string;
          duration_seconds: number;
        }[]
      >("echo360_sync_lectures", { canvasCourseId: subject.id });
      await upsertLectures(subject.id, data);
      await refreshLectures();
    } catch (e) {
      setSyncError(String(e));
    } finally {
      setSyncing(false);
    }
  };

  const handleSelectLecture = (lec: Lecture) => {
    setSelectedLecture(lec);
    // Feeds the "Recently visited" row on the subject home.
    recordRecent(subject.id, { kind: "lecture", ref: lec.id, title: lec.title });
  };

  const openAsPage = () => {
    if (!selectedLecture) return;
    const to = lecturePagePath(selectedLecture);
    addTab(to);
    navigate(to);
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
                const active = selectedLecture?.id === lec.id;
                const pl = progressLabel(lec);
                const prog = downloads.progress[lec.id];
                const isDown = !lec.video_path && isDownloading(downloads, lec.id);
                return (
                  <button
                    key={lec.id}
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

      {/* Player peek — expand promotes the lecture to its own standalone tab. */}
      {selectedLecture && (
        <PeekPanel
          key={selectedLecture.id}
          title={selectedLecture.title}
          onExpand={openAsPage}
          onClose={() => {
            // Closing the player is a stop; switching tabs is not.
            stopLecturePlayback();
            setSelectedLecture(null);
          }}
        >
          <LecturePlayer
            lecture={selectedLecture}
            onRefresh={refreshLectures}
            allowFullscreen={false}
          />
        </PeekPanel>
      )}
    </>
  );
}

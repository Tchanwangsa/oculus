import { useEffect, useRef, useState, useCallback } from "react";
import {
  Play, Pause, Download, CheckCircle2, Clock, Loader2,
  RefreshCw, FileText, ChevronRight, AlertCircle, Volume2,
} from "lucide-react";
import { invoke, convertFileSrc } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { cn } from "@/lib/utils";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  getLectures, upsertLectures, updateLectureVideoPath,
  updateLectureTranscriptPath, updateLectureProgress, markLectureComplete,
  getSubjects,
  type Lecture, type Subject,
} from "@/lib/db";

// ── VTT parser ────────────────────────────────────────────────────────────────

interface Cue { start: number; end: number; text: string }

function parseVtt(vtt: string): Cue[] {
  const cues: Cue[] = [];
  const blocks = vtt.split(/\n\n+/);
  for (const block of blocks) {
    const lines = block.trim().split('\n');
    const timeLine = lines.find(l => l.includes(' --> '));
    if (!timeLine) continue;
    const [startStr, endStr] = timeLine.split(' --> ');
    const start = vttToSecs(startStr?.trim() ?? '');
    const end   = vttToSecs(endStr?.split(' ')[0]?.trim() ?? '');
    const text  = lines.filter(l => !l.includes(' --> ')).join(' ').replace(/^\d+$/, '').trim();
    if (text && start >= 0) cues.push({ start, end, text });
  }
  return cues;
}

function vttToSecs(s: string): number {
  const parts = s.split(':');
  if (parts.length === 3) {
    return Number(parts[0]) * 3600 + Number(parts[1]) * 60 + Number(parts[2]);
  }
  if (parts.length === 2) {
    return Number(parts[0]) * 60 + Number(parts[1]);
  }
  return -1;
}

function fmtDuration(secs: number): string {
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const s = secs % 60;
  if (h > 0) return `${h}h ${m.toString().padStart(2, '0')}m`;
  return `${m}m ${s.toString().padStart(2, '0')}s`;
}

function fmtTime(secs: number): string {
  const m = Math.floor(secs / 60);
  const s = Math.floor(secs % 60);
  return `${m}:${s.toString().padStart(2, '0')}`;
}

function progressLabel(lec: Lecture): { text: string; color: string } {
  if (lec.completed) return { text: 'Done', color: 'text-success' };
  if (lec.progress_seconds > 5) {
    const left = Math.max(0, lec.duration_seconds - lec.progress_seconds);
    return { text: `${fmtTime(left)} left`, color: 'text-warning' };
  }
  return { text: 'Not watched', color: 'text-muted-foreground' };
}

function fmtDate(iso: string): string {
  const d = new Date(iso);
  return d.toLocaleDateString('en-AU', { weekday: 'short', day: 'numeric', month: 'short' });
}

// ── Download progress event ───────────────────────────────────────────────────

interface DlProgress { mediaId: string; percent: number; phase: string }

// ── Main component ────────────────────────────────────────────────────────────

export default function LecturesPage() {
  const [subjects, setSubjects] = useState<Subject[]>([]);
  const [selectedSubjectId, setSelectedSubjectId] = useState<number | null>(null);
  const [lectures, setLectures] = useState<Lecture[]>([]);
  const [selectedLecture, setSelectedLecture] = useState<Lecture | null>(null);
  const [syncing, setSyncing] = useState(false);
  const [syncError, setSyncError] = useState<string | null>(null);
  const [downloading, setDownloading] = useState<Set<string>>(new Set());
  const [dlProgress, setDlProgress] = useState<Record<string, DlProgress>>({});
  const [cues, setCues] = useState<Cue[]>([]);
  const [activeCueIdx, setActiveCueIdx] = useState(-1);
  const [isPlaying, setIsPlaying] = useState(false);
  const [speed, setSpeed] = useState(1.0);
  const [currentTime, setCurrentTime] = useState(0);

  const videoRef = useRef<HTMLVideoElement>(null);
  const transcriptRef = useRef<HTMLDivElement>(null);
  const progressSaveRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // ── Load subjects ─────────────────────────────────────────────────────────

  useEffect(() => {
    getSubjects().then(rows => {
      const current = rows.filter(s => s.is_current);
      setSubjects(current.length > 0 ? current : rows);
      if (current.length > 0) setSelectedSubjectId(current[0].id);
      else if (rows.length > 0) setSelectedSubjectId(rows[0].id);
    });
  }, []);

  // ── Load lectures from DB when subject changes ────────────────────────────

  const refreshLectures = useCallback(async (subjectId: number) => {
    const rows = await getLectures(subjectId);
    setLectures(rows);
    // Update selected lecture if still in list
    setSelectedLecture(prev => prev ? rows.find(r => r.id === prev.id) ?? null : null);
  }, []);

  useEffect(() => {
    if (selectedSubjectId != null) refreshLectures(selectedSubjectId);
  }, [selectedSubjectId, refreshLectures]);

  // ── Download progress events ──────────────────────────────────────────────

  useEffect(() => {
    const unsub = listen<DlProgress>("lecture-download-progress", (e) => {
      setDlProgress(prev => ({ ...prev, [e.payload.mediaId]: e.payload }));
      if (e.payload.phase === "complete" || e.payload.phase === "error") {
        setDownloading(prev => { const s = new Set(prev); s.delete(e.payload.mediaId); return s; });
        setTimeout(() => setDlProgress(prev => { const n = { ...prev }; delete n[e.payload.mediaId]; return n; }), 2000);
      }
    });
    return () => { unsub.then(f => f()); };
  }, []);

  // ── Sync lectures ─────────────────────────────────────────────────────────

  const handleSync = async () => {
    if (!selectedSubjectId) return;
    setSyncing(true);
    setSyncError(null);
    try {
      const data = await invoke<{ id: string; lesson_id: string; title: string; date: string; duration_seconds: number }[]>(
        "echo360_sync_lectures",
        { canvasCourseId: selectedSubjectId }
      );
      await upsertLectures(selectedSubjectId, data);
      await refreshLectures(selectedSubjectId);
    } catch (e) {
      setSyncError(String(e));
    } finally {
      setSyncing(false);
    }
  };

  // ── Download video ────────────────────────────────────────────────────────

  const handleDownloadVideo = async (lec: Lecture) => {
    setDownloading(prev => new Set(prev).add(lec.id));
    try {
      const result = await invoke<{ path: string; trim_offset: number }>("echo360_download_video", {
        mediaId: lec.id,
        lessonId: lec.lesson_id,
        canvasCourseId: lec.subject_id,
      });
      await updateLectureVideoPath(lec.id, result.path, result.trim_offset);
      if (selectedSubjectId != null) await refreshLectures(selectedSubjectId);
    } catch (e) {
      setSyncError(`Download failed: ${e}`);
      setDownloading(prev => { const s = new Set(prev); s.delete(lec.id); return s; });
    }
  };

  // ── Download transcript ───────────────────────────────────────────────────

  const handleDownloadTranscript = async (lec: Lecture) => {
    try {
      const path = await invoke<string>("echo360_download_transcript", {
        lessonId: lec.lesson_id,
        mediaId: lec.id,
        canvasCourseId: lec.subject_id,
      });
      await updateLectureTranscriptPath(lec.id, path);
      if (selectedSubjectId != null) await refreshLectures(selectedSubjectId);
    } catch (e) {
      setSyncError(`Transcript download failed: ${e}`);
    }
  };

  // ── Select lecture → load transcript, restore progress ────────────────────

  const handleSelectLecture = async (lec: Lecture) => {
    setSelectedLecture(lec);
    setActiveCueIdx(-1);
    setCues([]);
    setCurrentTime(0);
    setIsPlaying(false);

    if (lec.transcript_path) {
      try {
        const vtt = await invoke<string>("echo360_read_transcript", { path: lec.transcript_path });
        setCues(parseVtt(vtt));
      } catch { /* transcript unreadable */ }
    }

    // Seek past copyright notice (trim_offset) and restore saved progress
    if (lec.video_path) {
      const offset = lec.trim_offset ?? 0;
      const seekTo = offset + (lec.progress_seconds > 5 ? lec.progress_seconds : 0);
      if (seekTo > 0) {
        const onLoaded = () => {
          if (videoRef.current) videoRef.current.currentTime = seekTo;
          videoRef.current?.removeEventListener('loadedmetadata', onLoaded);
        };
        videoRef.current?.addEventListener('loadedmetadata', onLoaded);
      }
    }
  };

  // ── Video event handlers ──────────────────────────────────────────────────

  const handleTimeUpdate = () => {
    const v = videoRef.current;
    if (!v || !selectedLecture) return;
    const rawTime = v.currentTime;
    const offset = selectedLecture.trim_offset ?? 0;
    const t = Math.max(0, rawTime - offset); // user-facing time (after copyright skip)
    setCurrentTime(t);

    // Update active cue (transcript cues are already offset-adjusted)
    const idx = cues.findIndex(c => t >= c.start && t <= c.end);
    setActiveCueIdx(idx);
    if (idx >= 0 && transcriptRef.current) {
      const el = transcriptRef.current.querySelector(`[data-cue="${idx}"]`);
      el?.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
    }

    // Debounce progress save (save user-facing time)
    if (progressSaveRef.current) clearTimeout(progressSaveRef.current);
    progressSaveRef.current = setTimeout(async () => {
      if (!selectedLecture) return;
      await updateLectureProgress(selectedLecture.id, Math.floor(t));
      if (selectedLecture.duration_seconds > 0 && t >= selectedLecture.duration_seconds - 30) {
        await markLectureComplete(selectedLecture.id);
      }
      if (selectedSubjectId != null) await refreshLectures(selectedSubjectId);
    }, 5000);
  };

  const handleSeekToCue = (cue: Cue) => {
    if (videoRef.current) videoRef.current.currentTime = cue.start;
  };

  const togglePlay = () => {
    const v = videoRef.current;
    if (!v) return;
    if (v.paused) { v.play(); setIsPlaying(true); }
    else { v.pause(); setIsPlaying(false); }
  };

  const handleSpeedChange = (s: number) => {
    setSpeed(s);
    if (videoRef.current) videoRef.current.playbackRate = s;
  };

  // ── Computed ──────────────────────────────────────────────────────────────

  const selectedSubject = subjects.find(s => s.id === selectedSubjectId);
  const videoSrc = selectedLecture?.video_path
    ? convertFileSrc(selectedLecture.video_path)
    : null;

  // ── Render ────────────────────────────────────────────────────────────────

  return (
    <div className="flex h-full overflow-hidden">
      {/* ── Sidebar: subject picker + lecture list ── */}
      <div className="w-64 shrink-0 border-r border-border flex flex-col h-full overflow-hidden">
        {/* Subject selector */}
        {subjects.length > 1 && (
          <div className="px-3 py-2 border-b border-border shrink-0">
            <select
              value={selectedSubjectId ?? ''}
              onChange={e => setSelectedSubjectId(Number(e.target.value))}
              className="w-full text-xs bg-surface border border-border rounded-md px-2 py-1.5 text-foreground focus:outline-none"
            >
              {subjects.map(s => (
                <option key={s.id} value={s.id}>{s.code}</option>
              ))}
            </select>
          </div>
        )}

        {/* Header */}
        <div className="px-3 h-11 flex items-center justify-between border-b border-border shrink-0">
          <span className="text-xs font-semibold text-foreground">
            {selectedSubject?.code ?? 'Lectures'}
          </span>
          <div className="flex items-center gap-1">
            <span className="text-[10px] text-muted-foreground">{lectures.length}</span>
            <button
              onClick={handleSync}
              disabled={syncing || !selectedSubjectId}
              title="Sync lecture list"
              className="p-1 rounded hover:bg-surface text-muted-foreground hover:text-foreground transition-colors disabled:opacity-40"
            >
              <RefreshCw size={12} className={syncing ? 'animate-spin' : ''} />
            </button>
          </div>
        </div>

        {/* Lecture list */}
        <div className="flex-1 overflow-y-auto">
          {syncError && (
            <div className="mx-3 mt-2 px-2 py-2 rounded bg-destructive/10 text-destructive text-[10px] flex gap-1.5 items-start">
              <AlertCircle size={11} className="shrink-0 mt-0.5" />
              <span className="break-words">{syncError}</span>
            </div>
          )}
          {lectures.length === 0 ? (
            <div className="flex flex-col items-center justify-center h-32 gap-2 text-center px-4">
              <p className="text-xs text-muted-foreground">No lectures synced yet</p>
              <Button size="sm" variant="outline" className="text-xs gap-1.5 h-7" onClick={handleSync} disabled={syncing}>
                {syncing ? <Loader2 size={11} className="animate-spin" /> : <RefreshCw size={11} />}
                {syncing ? 'Syncing…' : 'Sync lectures'}
              </Button>
            </div>
          ) : (
            <div className="py-1">
              {lectures.map(lec => {
                const active = selectedLecture?.id === lec.id;
                const pl = progressLabel(lec);
                const isDown = downloading.has(lec.id);
                const prog = dlProgress[lec.id];
                return (
                  <button
                    key={lec.id}
                    onClick={() => handleSelectLecture(lec)}
                    className={cn(
                      "w-full text-left px-3 py-2 flex gap-2 items-start hover:bg-surface transition-colors",
                      active && "bg-surface-raised"
                    )}
                  >
                    <div className="mt-0.5 shrink-0">
                      {lec.completed
                        ? <CheckCircle2 size={13} className="text-success" />
                        : <div className={cn("w-3 h-3 rounded-full border-2", active ? "border-primary" : "border-muted-foreground/40")} />
                      }
                    </div>
                    <div className="min-w-0 flex-1">
                      <p className="text-[11px] font-medium text-foreground truncate leading-tight">
                        {lec.title}
                      </p>
                      <p className="text-[10px] text-muted-foreground mt-0.5">{fmtDate(lec.date)}</p>
                      <div className="flex items-center gap-1.5 mt-0.5">
                        <span className="text-[10px] text-muted-foreground flex items-center gap-0.5">
                          <Clock size={9} />{fmtDuration(lec.duration_seconds)}
                        </span>
                        <span className={cn("text-[10px]", pl.color)}>{pl.text}</span>
                      </div>
                      {isDown && prog && (
                        <div className="mt-1">
                          <div className="h-1 rounded-full bg-surface overflow-hidden">
                            <div
                              className="h-full bg-primary transition-all"
                              style={{ width: `${prog.percent}%` }}
                            />
                          </div>
                          <p className="text-[9px] text-muted-foreground mt-0.5 capitalize">{prog.phase}…</p>
                        </div>
                      )}
                    </div>
                    <div className="flex flex-col gap-1 items-end shrink-0 mt-0.5">
                      {lec.video_path
                        ? <CheckCircle2 size={10} className="text-success" />
                        : <Download size={10} className="text-muted-foreground/50" />
                      }
                      {lec.transcript_path
                        ? <CheckCircle2 size={10} className="text-success" />
                        : <FileText size={10} className="text-muted-foreground/50" />
                      }
                    </div>
                  </button>
                );
              })}
            </div>
          )}
        </div>
      </div>

      {/* ── Main: video player + transcript ── */}
      <div className="flex-1 flex flex-col overflow-hidden min-w-0">
        {!selectedLecture ? (
          <div className="flex-1 flex items-center justify-center flex-col gap-3 text-muted-foreground">
            <Play size={32} className="opacity-20" />
            <p className="text-sm">Select a lecture to watch</p>
          </div>
        ) : (
          <>
            {/* Video area */}
            <div className="flex-1 bg-black flex flex-col min-h-0">
              {videoSrc ? (
                <video
                  ref={videoRef}
                  src={videoSrc}
                  className="flex-1 w-full object-contain"
                  onTimeUpdate={handleTimeUpdate}
                  onPlay={() => setIsPlaying(true)}
                  onPause={() => setIsPlaying(false)}
                  onEnded={() => { setIsPlaying(false); }}
                />
              ) : (
                <div className="flex-1 flex flex-col items-center justify-center gap-4 text-white/60 p-6">
                  <p className="text-sm font-medium text-white">{selectedLecture.title}</p>
                  <p className="text-xs">{fmtDate(selectedLecture.date)} · {fmtDuration(selectedLecture.duration_seconds)}</p>
                  {downloading.has(selectedLecture.id) ? (
                    <div className="flex items-center gap-2 text-sm">
                      <Loader2 size={16} className="animate-spin" />
                      <span>
                        {dlProgress[selectedLecture.id]?.phase === 'trimming'
                          ? 'Trimming…'
                          : `Downloading… ${dlProgress[selectedLecture.id]?.percent ?? 0}%`}
                      </span>
                    </div>
                  ) : (
                    <Button
                      size="sm"
                      className="gap-2 bg-white/10 hover:bg-white/20 text-white border-white/20"
                      variant="outline"
                      onClick={() => handleDownloadVideo(selectedLecture)}
                    >
                      <Download size={14} /> Download video
                    </Button>
                  )}
                </div>
              )}
            </div>

            {/* Controls bar */}
            <div className="h-10 shrink-0 bg-card border-t border-border flex items-center px-3 gap-3">
              <button
                onClick={togglePlay}
                disabled={!videoSrc}
                className="p-1 rounded hover:bg-surface text-foreground disabled:opacity-30 transition-colors"
              >
                {isPlaying ? <Pause size={14} /> : <Play size={14} />}
              </button>

              <span className="text-[11px] text-muted-foreground font-mono min-w-[80px]">
                {fmtTime(Math.floor(currentTime))} / {fmtTime(selectedLecture.duration_seconds)}
              </span>

              {/* Scrub bar */}
              <div className="flex-1 h-1.5 bg-surface-raised rounded-full overflow-hidden cursor-pointer"
                onClick={e => {
                  const rect = e.currentTarget.getBoundingClientRect();
                  const pct = (e.clientX - rect.left) / rect.width;
                  const offset = selectedLecture.trim_offset ?? 0;
                  if (videoRef.current) videoRef.current.currentTime = offset + pct * selectedLecture.duration_seconds;
                }}
              >
                <div
                  className="h-full bg-primary rounded-full transition-none"
                  style={{ width: `${selectedLecture.duration_seconds ? (currentTime / selectedLecture.duration_seconds) * 100 : 0}%` }}
                />
              </div>

              {/* Speed */}
              <div className="flex items-center gap-1">
                <Volume2 size={11} className="text-muted-foreground" />
                <select
                  value={speed}
                  onChange={e => handleSpeedChange(Number(e.target.value))}
                  className="text-[11px] bg-transparent text-foreground focus:outline-none cursor-pointer"
                >
                  {[0.5, 0.75, 1, 1.25, 1.5, 1.75, 2].map(s => (
                    <option key={s} value={s}>{s}×</option>
                  ))}
                </select>
              </div>

              {/* Action buttons */}
              <div className="flex items-center gap-1 ml-1">
                {!selectedLecture.video_path && (
                  <button
                    onClick={() => handleDownloadVideo(selectedLecture)}
                    disabled={downloading.has(selectedLecture.id)}
                    title="Download video"
                    className="p-1 rounded hover:bg-surface text-muted-foreground hover:text-foreground disabled:opacity-30 transition-colors"
                  >
                    {downloading.has(selectedLecture.id)
                      ? <Loader2 size={13} className="animate-spin" />
                      : <Download size={13} />
                    }
                  </button>
                )}
                {!selectedLecture.transcript_path && (
                  <button
                    onClick={() => handleDownloadTranscript(selectedLecture)}
                    title="Download transcript"
                    className="p-1 rounded hover:bg-surface text-muted-foreground hover:text-foreground transition-colors"
                  >
                    <FileText size={13} />
                  </button>
                )}
                {selectedLecture.completed && (
                  <Badge variant="success" className="text-[10px] h-5">Done</Badge>
                )}
              </div>
            </div>

            {/* Transcript panel */}
            {cues.length > 0 && (
              <div className="h-44 shrink-0 border-t border-border bg-background flex flex-col">
                <div className="px-3 h-8 flex items-center border-b border-border shrink-0">
                  <span className="text-[11px] font-semibold text-foreground">Transcript</span>
                  <span className="text-[10px] text-muted-foreground ml-auto">{cues.length} cues</span>
                </div>
                <div ref={transcriptRef} className="flex-1 overflow-y-auto px-3 py-2 space-y-0.5">
                  {cues.map((cue, i) => (
                    <button
                      key={i}
                      data-cue={i}
                      onClick={() => handleSeekToCue(cue)}
                      className={cn(
                        "w-full text-left text-[11px] px-2 py-1 rounded flex gap-2 items-start transition-colors",
                        i === activeCueIdx
                          ? "bg-primary/10 text-primary"
                          : "text-muted-foreground hover:text-foreground hover:bg-surface"
                      )}
                    >
                      <span className="font-mono text-[10px] shrink-0 pt-px w-10 text-right opacity-60">
                        {fmtTime(Math.floor(cue.start))}
                      </span>
                      <span className="flex-1">{cue.text}</span>
                      <ChevronRight size={10} className="shrink-0 mt-1 opacity-40" />
                    </button>
                  ))}
                </div>
              </div>
            )}
          </>
        )}
      </div>
    </div>
  );
}

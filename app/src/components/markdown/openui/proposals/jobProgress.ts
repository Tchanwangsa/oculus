import type { SyncProgress } from "@/stores/sync/syncStore";
import type { ParseJob, ParseFailure } from "@/stores/sync/parseStore";
import { CHAPTER_PHASE_LABEL, type ChapterPhase } from "@/lib/lectures/chapters";
import { object, type Row } from "./model";

export interface JobProgressValue {
  label: string;
  done?: number;
  total?: number;
  percent?: number;
  error?: string;
  sync?: SyncProgress;
  cancel?: { command: string; args?: Record<string, unknown> };
}
export interface JobProgressSources {
  sync: { runId: number | null; scraping: boolean; progress: SyncProgress | null };
  parses: { statuses: Record<string, string>; jobs: Record<string, ParseJob>; failures: Record<string, ParseFailure> };
  event?: { name: string; payload: Row };
}
const percent = (done: number, total: number) => total > 0 ? Math.max(0, Math.min(100, done / total * 100)) : undefined;

/** Identity gates precede every source read; a card never adopts the latest sync. */
export function jobProgress(kind: string, state: Row, sources: JobProgressSources): JobProgressValue {
  const job = object(state.job);
  const event = sources.event;
  if (kind === "Sync") {
    const ownsRun = typeof job.run_id === "number" && sources.sync.runId === job.run_id && sources.sync.scraping;
    const tagged = event?.name === "scrape-progress" && event.payload.runId === job.run_id && typeof job.run_id === "number";
    const progress = tagged ? event.payload as unknown as SyncProgress : ownsRun ? sources.sync.progress : null;
    return { label: state.lectures ? "Refreshing lecture lists…" : "Starting sync…", sync: progress ?? undefined,
      cancel: ownsRun && !state.lectures ? { command: "cancel_scrape" } : undefined };
  }
  if (kind === "Reparse") {
    const files = Array.isArray(state.files) ? state.files.filter((v): v is string => typeof v === "string") : [];
    const done = files.filter((file) => sources.parses.statuses[file] === "quality").length;
    const active = files.find((file) => sources.parses.jobs[file]);
    const started = typeof job.started_at === "string" ? Date.parse(job.started_at) : Infinity;
    const failure = files.map((file) => sources.parses.failures[file]).find((failure) => failure && failure.at >= started);
    return { label: active ? `Parsing ${active.split("/").pop()}` : done === files.length ? "Embedding parsed files…" : "Waiting for parsing…", done, total: files.length, percent: percent(done, files.length), error: failure?.message };
  }
  const p = event?.payload ?? {};
  if (kind === "Transcribe") {
    if (event?.name !== "transcribe-progress" || p?.path !== state.file) return { label: "Preparing transcription…" };
    const chunk = Number(p.chunk ?? 0), chunks = Number(p.chunks ?? 0);
    // A chunk is announced before it is transcribed.
    const done = p.phase === "complete" ? chunks : Math.max(0, chunk - 1);
    return { label: p.phase === "extracting" ? "Extracting audio…" : `Transcribing${p.engine ? ` · ${p.engine}` : ""}`, done, total: chunks, percent: percent(done, chunks) };
  }
  if (kind === "Download") {
    const item = String(state.item);
    const matches = item === "aligner" ? event?.name === "studio-aligner-progress" : item === "voices" ? event?.name === "studio-voice-progress" || (event?.name === "studio-voice-file-progress" && ["af_heart", "am_michael"].includes(String(p?.id))) : event?.name === "studio-voice-file-progress" && p?.id === item;
    const cancel = item === "aligner" ? { command: "studio_aligner_cancel" } : item === "voices" ? event?.name === "studio-voice-file-progress" && matches ? { command: "studio_voice_cancel", args: { id: p?.id } } : { command: "studio_voice_packs_cancel" } : { command: "studio_voice_cancel", args: { id: item } };
    return { label: matches && p?.id ? `Downloading ${p.id}` : `Downloading ${item}…`, percent: matches ? percent(Number(p?.received), Number(p?.total)) : undefined, cancel: (matches && p?.phase === "downloading") ? cancel : undefined };
  }
  if (kind === "LectureChapters") {
    if (event?.name !== "lecture-chapter-progress" || p?.lectureId !== state.lecture) return { label: "Preparing lecture chapters…" };
    const done = typeof p.done === "number" ? p.done : undefined, total = typeof p.total === "number" ? p.total : undefined;
    return { label: CHAPTER_PHASE_LABEL[p.phase as ChapterPhase] ?? "Finding chapters…", done, total, percent: done != null && total != null ? percent(done, total) : undefined };
  }
  // LectureEnd emits only a terminal event; its configured model reads the transcript.
  return { label: "Reading the lecture transcript…" };
}

export function jobProgressEvents(kind: string): string[] {
  return { Sync: ["scrape-progress"], Reparse: [], Transcribe: ["transcribe-progress"], Download: ["studio-aligner-progress", "studio-voice-progress", "studio-voice-file-progress"], LectureChapters: ["lecture-chapter-progress"], LectureEnd: [] }[kind] ?? [];
}

export function matchesJobProgressEvent(kind: string, state: Row, name: string, payload: Row): boolean {
  if (!jobProgressEvents(kind).includes(name)) return false;
  if (kind === "Sync") return typeof object(state.job).run_id === "number" && payload.runId === object(state.job).run_id;
  if (kind === "Transcribe") return payload.path === state.file;
  if (kind === "LectureChapters") return payload.lectureId === state.lecture;
  if (kind === "Download") return state.item === "aligner" ? name === "studio-aligner-progress" : state.item === "voices" ? name === "studio-voice-progress" || name === "studio-voice-file-progress" && ["af_heart", "am_michael"].includes(String(payload.id)) : name === "studio-voice-file-progress" && payload.id === state.item;
  return false;
}

import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  updateLectureVideoPath,
  updateLectureTranscriptPath,
  type Lecture,
  type SourceNum,
} from "@/lib/db";
import type { DlProgress } from "@/lib/lectures";

/**
 * Lecture download state, global on purpose: a download keeps running in Rust
 * after the player peek closes, so the "is this downloading?" answer cannot
 * live in any one component. Rows and players all read from here.
 *
 * Keyed by media id *and* source: a lecture's screen and camera streams are
 * separate downloads that can run at the same time, and a single key would
 * have the second one overwriting the first one's bar.
 */
export const dlKey = (id: string, source: SourceNum = 1) => `${id}:${source}`;

interface LectureDownloadsState {
  /** `dlKey` -> live progress; cleared shortly after complete/error. */
  progress: Record<string, DlProgress>;
  /** `dlKey` -> true from the download call until it settles. */
  active: Record<string, boolean>;
}

export const useLectureDownloads = create<LectureDownloadsState>(() => ({
  progress: {},
  active: {},
}));

/** Fired (detail: lecture id) after the video/transcript paths hit the DB. */
export const LECTURE_DOWNLOADED_EVENT = "oculus:lecture-downloaded";

function setActive(key: string, on: boolean) {
  useLectureDownloads.setState((s) => {
    const active = { ...s.active };
    if (on) active[key] = true;
    else delete active[key];
    return { active };
  });
}

/**
 * Download one of a lecture's streams, and on the first one its transcript
 * alongside (a transcript failure is silent — not every lecture has one). The
 * camera stream shares the lecture's transcript, so source 2 never fetches it.
 * No-op if that source is already running. Throws on video download failure.
 */
export async function downloadLecture(
  lecture: Lecture,
  source: SourceNum = 1,
): Promise<void> {
  const key = dlKey(lecture.id, source);
  if (useLectureDownloads.getState().active[key]) return;
  setActive(key, true);
  try {
    const path = await invoke<string>("echo360_download_video", {
      mediaId: lecture.id,
      lessonId: lecture.lesson_id,
      canvasCourseId: lecture.subject_id,
      source,
    });
    await updateLectureVideoPath(lecture.id, path, source);

    if (source === 1 && !lecture.transcript_path) {
      try {
        const tPath = await invoke<string>("echo360_download_transcript", {
          lessonId: lecture.lesson_id,
          mediaId: lecture.id,
          canvasCourseId: lecture.subject_id,
        });
        await updateLectureTranscriptPath(lecture.id, tPath);
      } catch {
        /* no transcript published for this lecture */
      }
    }

    window.dispatchEvent(
      new CustomEvent(LECTURE_DOWNLOADED_EVENT, { detail: lecture.id }),
    );
  } finally {
    setActive(key, false);
  }
}

/** True while `id`'s given source is mid-download (read via the store hook). */
export function isDownloading(
  s: LectureDownloadsState,
  id: string,
  source: SourceNum = 1,
): boolean {
  const key = dlKey(id, source);
  const p = s.progress[key];
  return !!s.active[key] || (p != null && p.phase !== "complete" && p.phase !== "error");
}

/** Subscribe the store to backend progress events. Mount once at the app root. */
export function watchLectureDownloads(): () => void {
  const unsub = listen<DlProgress>("lecture-download-progress", (e) => {
    const p = e.payload;
    const key = dlKey(p.mediaId, p.source);
    useLectureDownloads.setState((s) => ({ progress: { ...s.progress, [key]: p } }));
    if (p.phase === "complete" || p.phase === "error") {
      setTimeout(() => {
        useLectureDownloads.setState((s) => {
          const progress = { ...s.progress };
          delete progress[key];
          return { progress };
        });
      }, 2000);
    }
  });
  return () => {
    unsub.then((f) => f());
  };
}

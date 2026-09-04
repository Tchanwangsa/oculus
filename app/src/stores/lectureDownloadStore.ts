import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  updateLectureVideoPath,
  updateLectureTranscriptPath,
  type Lecture,
} from "@/lib/db";
import type { DlProgress } from "@/lib/lectures";

/**
 * Lecture download state, global on purpose: a download keeps running in Rust
 * after the player peek closes, so the "is this downloading?" answer cannot
 * live in any one component. Rows and players all read from here.
 */
interface LectureDownloadsState {
  /** mediaId -> live progress; cleared shortly after complete/error. */
  progress: Record<string, DlProgress>;
  /** mediaId -> true from the download call until it settles. */
  active: Record<string, boolean>;
}

export const useLectureDownloads = create<LectureDownloadsState>(() => ({
  progress: {},
  active: {},
}));

/** Fired (detail: lecture id) after the video/transcript paths hit the DB. */
export const LECTURE_DOWNLOADED_EVENT = "oculus:lecture-downloaded";

function setActive(id: string, on: boolean) {
  useLectureDownloads.setState((s) => {
    const active = { ...s.active };
    if (on) active[id] = true;
    else delete active[id];
    return { active };
  });
}

/**
 * Download a lecture's video, then its transcript alongside it (a transcript
 * failure is silent — not every lecture has one). No-op if already running.
 * Throws on video download failure.
 */
export async function downloadLecture(lecture: Lecture): Promise<void> {
  if (useLectureDownloads.getState().active[lecture.id]) return;
  setActive(lecture.id, true);
  try {
    const path = await invoke<string>("echo360_download_video", {
      mediaId: lecture.id,
      lessonId: lecture.lesson_id,
      canvasCourseId: lecture.subject_id,
    });
    await updateLectureVideoPath(lecture.id, path);

    if (!lecture.transcript_path) {
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
    setActive(lecture.id, false);
  }
}

/** True while `id` is mid-download (call in a component via the store hook). */
export function isDownloading(s: LectureDownloadsState, id: string): boolean {
  const p = s.progress[id];
  return !!s.active[id] || (p != null && p.phase !== "complete" && p.phase !== "error");
}

/** Subscribe the store to backend progress events. Mount once at the app root. */
export function watchLectureDownloads(): () => void {
  const unsub = listen<DlProgress>("lecture-download-progress", (e) => {
    const p = e.payload;
    useLectureDownloads.setState((s) => ({ progress: { ...s.progress, [p.mediaId]: p } }));
    if (p.phase === "complete" || p.phase === "error") {
      setTimeout(() => {
        useLectureDownloads.setState((s) => {
          const progress = { ...s.progress };
          delete progress[p.mediaId];
          return { progress };
        });
      }, 2000);
    }
  });
  return () => {
    unsub.then((f) => f());
  };
}

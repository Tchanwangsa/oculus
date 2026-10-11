import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useWindowEvent } from "@/hooks/backend/useEvents";
import { updateLectureTranscriptPath, type Lecture } from "@/lib/db";
import { parseVtt, type Cue } from "@/lib/lectures/media";
import { LECTURE_DOWNLOADED_EVENT } from "@/stores/lectures/lectureDownloadStore";
import {
  TRANSCRIBED_EVENT,
  settleTranscription,
  useTranscription,
  type TranscribedDetail,
} from "@/hooks/lectures/useTranscription";
import { usePlayerPrefs } from "@/stores/lectures/playerPrefsStore";

interface LectureTranscriptArgs {
  lecture: Lecture;
  onRefresh: () => void;
  setError: (message: string | null) => void;
}

/** The lecture's cues: loading, downloading, transcribing, and what the T key
 *  does about them. */
export function useLectureTranscript({ lecture, onRefresh, setError }: LectureTranscriptArgs) {
  const [cues, setCues] = useState<Cue[]>([]);
  const transcriptVisible = usePlayerPrefs((s) => s.transcriptVisible);
  const dockTab = usePlayerPrefs((s) => s.dockTab);
  const setPrefs = usePlayerPrefs((s) => s.set);

  const loadTranscript = useCallback(async (path: string) => {
    try {
      const vtt = await invoke<string>("echo360_read_transcript", { path });
      setCues(parseVtt(vtt));
    } catch {
      /* transcript unreadable */
    }
  }, []);

  // Downloads the transcript alongside the video; both land in the DB before
  // it resolves, so the refresh picks the paths up.
  const handleDownloadTranscript = async () => {
    setError(null);
    try {
      const path = await invoke<string>("echo360_download_transcript", {
        lessonId: lecture.lesson_id,
        mediaId: lecture.id,
        canvasCourseId: lecture.subject_id,
      });
      await updateLectureTranscriptPath(lecture.id, path);
      onRefresh();
      await loadTranscript(path);
      setPrefs({ transcriptVisible: true });
    } catch (e) {
      setError(`Transcript download failed: ${e}`);
      // Echo360 has none: the Transcript tab offers Transcribe instead.
      setPrefs({ transcriptVisible: true, dockTab: "transcript" });
    }
  };

  // T does what the dock button does: parse a transcript on disk but not loaded,
  // fetch one if the *preference* is the transcript tab (it outlives
  // `tabInFront` dropping that tab), otherwise show or hide the dock.
  const toggleTranscript = () => {
    if (lecture.transcript_path && cues.length === 0) {
      loadTranscript(lecture.transcript_path);
    } else if (!lecture.transcript_path && dockTab === "transcript") {
      handleDownloadTranscript();
    } else {
      setPrefs({ transcriptVisible: !transcriptVisible });
    }
  };

  // Reset per lecture: fresh transcript; the player resets its own clock.
  useEffect(() => {
    setCues([]);
    setError(null);

    if (lecture.transcript_path) {
      // A finished run waits, as a spinner, until its cues are read.
      const video = lecture.video_path;
      loadTranscript(lecture.transcript_path).finally(
        () => video && settleTranscription(video),
      );
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [lecture.id, lecture.transcript_path, lecture.video_path]);

  /** Records a finished run's VTT as the transcript, as a download does;
   *  it runs even if this player has unmounted by then. */
  const lectureId = lecture.id;
  const recordTranscript = useCallback(
    async (vtt: string) => {
      await updateLectureTranscriptPath(lectureId, vtt);
      window.dispatchEvent(new CustomEvent(LECTURE_DOWNLOADED_EVENT, { detail: lectureId }));
    },
    [lectureId],
  );

  /** A finished run keeps the Transcript tab on its spinner until the cues
   *  load, so the tab neither drops nor offers a second run. */
  const { run: transcribeRun } = useTranscription(lecture.video_path);
  const awaitingCues = transcribeRun?.phase === "done" && cues.length === 0;

  useWindowEvent(TRANSCRIBED_EVENT, (e) => {
    const { path } = (e as CustomEvent<TranscribedDetail>).detail;
    if (path === lecture.video_path) onRefresh();
  });

  return { cues, toggleTranscript, awaitingCues, recordTranscript };
}

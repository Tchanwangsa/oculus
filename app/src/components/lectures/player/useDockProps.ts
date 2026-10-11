import { useCallback, useMemo, type RefObject } from "react";
import type { Lecture } from "@/lib/db";
import { fmtClockSecs, spanAt, type Cue } from "@/lib/lectures/media";
import { lectureGrabFrames } from "@/lib/lectures";
import type { useLectureChapters } from "@/hooks/lectures/useLectureChapters";
import type { ChaptersPanelProps } from "@/components/lectures/ChaptersPanel";
import type { LectureChatPanelProps } from "@/components/lectures/LectureChatPanel";
import type { TranscribeEmptyProps } from "@/components/media/TranscribeEmpty";
import { SOURCE_HINT } from "@/components/lectures/SourceControls";
import { MOMENT_TRANSCRIPT_S } from "@/components/lectures/player/constants";

interface DockPropsArgs {
  lecture: Lecture;
  cues: Cue[];
  chapterState: ReturnType<typeof useLectureChapters>;
  chapterStarts: number[];
  activeChapterIdx: number;
  atRef: RefObject<number>;
  duration: number;
  seek: (seconds: number) => void;
  endError: string | null;
  onRetryEnd: () => void;
  awaitingCues: boolean;
  recordTranscript: (vtt: string) => Promise<void>;
}

/** What each dock tab is handed — one memoised bag per tab, because
 *  `TranscriptPanel` is memo'd against a player that re-renders on every
 *  `timeupdate`. */
export function useDockProps({
  lecture,
  cues,
  chapterState,
  chapterStarts,
  activeChapterIdx,
  atRef,
  duration,
  seek,
  endError,
  onRetryEnd,
  awaitingCues,
  recordTranscript,
}: DockPropsArgs) {
  const chaptersProps: ChaptersPanelProps = useMemo(
    () => ({
      chapters: chapterState.chapters,
      activeIdx: activeChapterIdx,
      atRef,
      duration,
      status: chapterState.status,
      error: chapterState.error,
      since: chapterState.since,
      progress: chapterState.progress,
      busy: chapterState.busy,
      downloaded: !!lecture.video_path,
      onSeek: seek,
      onFind: chapterState.find,
      endError,
      onRetryEnd,
    }),
    [
      chapterState.chapters,
      chapterState.status,
      chapterState.error,
      chapterState.since,
      chapterState.progress,
      chapterState.busy,
      chapterState.find,
      activeChapterIdx,
      atRef,
      duration,
      lecture.video_path,
      seek,
      endError,
      onRetryEnd,
    ],
  );

  /** The Transcript tab while the lecture has no transcript: Transcribe runs
   *  on the main recording. */
  const transcribeProps: TranscribeEmptyProps | null = useMemo(
    () =>
      lecture.transcript_path && !awaitingCues
        ? null
        : { path: lecture.video_path, after: recordTranscript },
    [lecture.transcript_path, awaitingCues, lecture.video_path, recordTranscript],
  );

  /**
   * The moment a dock message carries, as `SendOptions.context`
   * (`docs/harness.md`): chapter, the last minute of cues inline, and a frame of
   * *every* downloaded stream — the teaching may be on either source. A frame
   * that cannot be grabbed is dropped, never the message.
   */
  const buildMoment = useCallback(
    async (at: number): Promise<string> => {
      const parts: string[] = [
        `The student is at ${fmtClockSecs(at, true)} of this recording (second ${at}).`,
      ];

      const idx = spanAt(chapterStarts, at);
      const chapter = idx >= 0 ? chapterState.chapters[idx] : null;
      if (chapter) {
        parts.push(
          `That is inside chapter ${idx + 1}, "${chapter.title}", which starts at ${fmtClockSecs(chapter.start_seconds, true)}.`,
        );
      }

      const from = Math.max(0, at - MOMENT_TRANSCRIPT_S);
      const said = cues
        .filter((c) => c.start >= from && c.start <= at)
        .map((c) => c.text)
        .join(" ")
        .trim();
      if (said) {
        parts.push(
          `What was said between ${fmtClockSecs(from, true)} and ${fmtClockSecs(at, true)}:\n\n${said}`,
        );
      }

      const frames = await lectureGrabFrames(lecture.id, at).catch(() => []);
      if (frames.length) {
        const shots = frames
          .map((f) => `- source ${f.source} (usually the ${SOURCE_HINT[f.source].toLowerCase()}): \`${f.path}\``)
          .join("\n");
        parts.push(
          frames.length > 1
            ? `Frames of every stream of this capture at that second. Echo360 numbers the streams rather than naming them and either may be the one being taught from — a board is often only on the camera, slides only on the screen capture — so read all of them:\n\n${shots}`
            : `A frame of the recording at that second:\n\n${shots}`,
        );
      }

      return parts.join("\n\n");
    },
    [lecture.id, cues, chapterStarts, chapterState.chapters],
  );

  // Stable across `timeupdate`: the playhead travels by ref.
  const chatProps: LectureChatPanelProps = useMemo(
    () => ({ lectureId: lecture.id, atRef, buildMoment }),
    [lecture.id, atRef, buildMoment],
  );

  return { chaptersProps, transcribeProps, chatProps };
}

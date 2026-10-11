import { useEffect, useMemo, useState } from "react";
import { videoPathFor, type Lecture, type SourceNum } from "@/lib/db";
import { mediaSrc } from "@/lib/lectures/media";
import {
  dlKey,
  isDownloading,
  useLectureDownloads,
} from "@/stores/lectures/lectureDownloadStore";
import {
  SOURCES,
  type SourceState,
  type SourceStates,
} from "@/components/lectures/SourceControls";

/** Where each source can be played from, and its download state. */
export function useLectureSources(lecture: Lecture) {
  // Global: a download outlives this component (it runs in Rust).
  const downloading = useLectureDownloads((s) => isDownloading(s, lecture.id));
  const downloadingSecond = useLectureDownloads((s) => isDownloading(s, lecture.id, 2));
  const dlProgress = useLectureDownloads((s) => s.progress[dlKey(lecture.id, 1)] ?? null);
  const secondDlProgress = useLectureDownloads((s) => s.progress[dlKey(lecture.id, 2)] ?? null);

  // Served over localhost HTTP, not convertFileSrc — WebKit's media stack
  // refuses custom-scheme (asset://) sources outright. See lib/lectures/media/.
  // Tagged with their lecture: when the route moves on to another one, the
  // last lecture's URLs must not be handed to the new one while these resolve.
  const [resolved, setResolved] = useState<{ id: string } & Record<SourceNum, string | null>>({
    id: lecture.id,
    1: null,
    2: null,
  });
  useEffect(() => {
    let stale = false;
    const id = lecture.id;
    Promise.all(
      SOURCES.map(async (n) => {
        const path = videoPathFor(lecture, n);
        return [n, path ? await mediaSrc(path) : null] as const;
      }),
    ).then((pairs) => {
      if (!stale) {
        setResolved({ id, 1: pairs[0][1], 2: pairs[1][1] });
      }
    });
    return () => {
      stale = true;
    };
  }, [lecture.id, lecture.video_path, lecture.video2_path]); // eslint-disable-line react-hooks/exhaustive-deps
  const urls: Record<SourceNum, string | null> =
    resolved.id === lecture.id ? resolved : { 1: null, 2: null };

  /** Downloaded / downloading, per source, for the two source controls. */
  const sources: SourceStates = useMemo(() => {
    const of = (n: SourceNum): SourceState => {
      const p = n === 1 ? dlProgress : secondDlProgress;
      return {
        ready: !!videoPathFor(lecture, n),
        busy: n === 1 ? downloading : downloadingSecond,
        percent: p?.percent ?? 0,
        phase: p?.phase ?? "",
      };
    };
    return { 1: of(1), 2: of(2) };
  }, [dlProgress, secondDlProgress, downloading, downloadingSecond, lecture.video_path, lecture.video2_path]);

  return { urls, sources, downloading, dlProgress };
}

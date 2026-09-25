import { useSidePanelStore } from "@/stores/sidePanelStore";
import { PanelHeader } from "@/components/panel/PanelHeader";
import { LecturePlayer } from "@/components/lectures/LecturePlayer";
import { lecturePagePath, LECTURES_CHANGED_EVENT } from "@/lib/lectures";
import type { Lecture } from "@/lib/db";
import { stopLecturePlayback } from "@/lib/lecturePlayback";

/**
 * A lecture open in the side panel. Expanding hands the video elements over
 * to the page (`lib/lecturePlayback.ts`) rather than interrupting playback.
 * Fullscreen and the dock are off here; the full page has both.
 */
export default function LecturePanel({
  lecture,
  paneId,
  onExpand,
}: {
  lecture: Lecture;
  paneId: number;
  onExpand: (path: string, newTab: boolean) => void;
}) {
  const close = useSidePanelStore((s) => s.close);

  // Stop on the close control, not on unmount: switching tabs unmounts the
  // body and playback must survive that.
  const closeAndStop = () => {
    stopLecturePlayback();
    close(paneId);
  };

  return (
    <>
      <PanelHeader
        title={lecture.title}
        onExpand={(newTab) => onExpand(lecturePagePath(lecture), newTab)}
        onClose={closeAndStop}
      />
      <div className="flex-1 min-h-0 overflow-hidden flex flex-col">
        <LecturePlayer
          lecture={lecture}
          onRefresh={() =>
            window.dispatchEvent(new CustomEvent(LECTURES_CHANGED_EVENT))
          }
          allowFullscreen={false}
          allowDock={false}
          host="panel"
        />
      </div>
    </>
  );
}

import { useSidePanelStore } from "@/stores/sidePanelStore";
import { PanelHeader } from "@/components/panel/PanelHeader";
import { LecturePlayer } from "@/components/lectures/LecturePlayer";
import { lecturePagePath, LECTURES_CHANGED_EVENT } from "@/lib/lectures";
import type { Lecture } from "@/lib/db";
import { stopLecturePlayback } from "@/lib/lecturePlayback";

/**
 * A lecture open in the side panel. Expanding promotes it to the lecture page
 * — in this tab, or in a new one on ⌘-click — which either way is a handover
 * of the video elements rather than an interruption (`lib/lecturePlayback.ts`).
 *
 * Fullscreen stays off here for the same reason it always did: the panel is
 * furniture beside a page that stays where it is, and an element-fullscreen
 * player inside it would have to escape a Radix portal to do anything. The
 * dock is off for a reason of its own — a panel beside the video inside a
 * panel beside the page leaves neither of them readable (`allowDock` in
 * `LecturePlayer`). The page route, where both default on, is where they live,
 * and the expand control in the header above is the way there.
 */
export default function LecturePanel({
  lecture,
  tabId,
  onExpand,
}: {
  lecture: Lecture;
  tabId: number;
  onExpand: (path: string, newTab: boolean) => void;
}) {
  const close = useSidePanelStore((s) => s.close);

  // Closing the player is a stop; switching tabs is not — which is why this
  // sits on the close control and not on an unmount effect. The panel
  // unmounts its body whenever another tab comes forward, and a lecture is
  // meant to keep playing through that.
  const closeAndStop = () => {
    stopLecturePlayback();
    close(tabId);
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

import { useSidePanelStore } from "@/stores/sidePanelStore";
import { useTabStore } from "@/stores/tabStore";
import { PanelHeader } from "@/components/panel/PanelHeader";
import { LecturePlayer } from "@/components/lectures/LecturePlayer";
import { lecturePagePath, LECTURES_CHANGED_EVENT } from "@/lib/lectures";
import type { Lecture } from "@/lib/db";
import { stopLecturePlayback } from "@/lib/lecturePlayback";

/**
 * A lecture open in the side panel.
 *
 * Fullscreen stays off here for the same reason it always did: the panel is
 * furniture beside a page that stays where it is, and an element-fullscreen
 * player inside it would have to escape a Radix portal to do anything. The
 * page route (`allowFullscreen` defaulting on) is where that lives.
 */
export default function LecturePanel({
  lecture,
  tabId,
}: {
  lecture: Lecture;
  tabId: number;
}) {
  const addTab = useTabStore((s) => s.addTab);
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
        onExpand={() => {
          addTab(lecturePagePath(lecture));
          close(tabId);
        }}
        onClose={closeAndStop}
      />
      <div className="flex-1 min-h-0 overflow-hidden flex flex-col">
        <LecturePlayer
          lecture={lecture}
          onRefresh={() =>
            window.dispatchEvent(new CustomEvent(LECTURES_CHANGED_EVENT))
          }
          allowFullscreen={false}
        />
      </div>
    </>
  );
}

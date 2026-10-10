import type { PipCorner } from "@/hooks/lectures/useSourceLayout";
import type { DockTab } from "@/stores/lectures/playerPrefsStore";

/** Seconds of transcript before the playhead that a chat moment carries. */
export const MOMENT_TRANSCRIPT_S = 60;

/** What the dock button calls the tab in front. */
export const DOCK_TAB_NOUN: Record<DockTab, string> = {
  chapters: "chapters",
  transcript: "transcript",
  chat: "chat",
};

/** Where a corner handle sits, and which way it resizes from there. */
export const CORNER_STYLE: Record<PipCorner, string> = {
  nw: "left-0 top-0 cursor-nwse-resize",
  ne: "right-0 top-0 cursor-nesw-resize",
  sw: "bottom-0 left-0 cursor-nesw-resize",
  se: "bottom-0 right-0 cursor-nwse-resize",
};

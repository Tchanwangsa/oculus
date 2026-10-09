import type { Dock } from "@/hooks/lectures/useTranscriptDock";

/** Turns `SidebarSimple` to face the dock's edge; the glyph's divider is on
 *  the left, so left is unrotated. */
export const DOCK_ICON_FACING: Record<Dock, string> = {
  left: "",
  right: "rotate-180",
  top: "rotate-90",
  bottom: "-rotate-90",
};

/** Is a popover open? Its dismissing click must not also play/pause or seek.
 *  Radix dismisses on `click` at `document`, so during the video's handler the
 *  panel is still `data-state=open`. */
export const panelOnScreen = () =>
  !!document.querySelector('[data-slot="popover-content"][data-state="open"]');

export const NO_STARTS: number[] = [];

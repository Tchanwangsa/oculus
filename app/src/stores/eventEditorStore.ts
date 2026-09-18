import { create } from "zustand";
import type { CalEvent } from "@/lib/calendar";

/**
 * The "new / edit event" dialog, as a store rather than page state, because
 * the two things that raise it are not in one subtree: the Calendar header's
 * New event button, and the Edit on an event's own card — which is
 * `EventPopover`, rendered by all three calendar views *and* by Home's Today
 * list. Both ask through here and one dialog in `AppLayout` answers, the same
 * shape `leaveLectureStore` uses.
 */
interface EventEditorState {
  open: boolean;
  /** The local event being edited. `null` while creating a new one. */
  editing: CalEvent | null;
  /**
   * The day a new event should start on.
   *
   * The calendar's anchor, not today: someone paging through October and
   * pressing New event means a day in October, and a dialog that opened on
   * today would have them navigating back to where they already were.
   */
  day: Date | null;
  close: () => void;
}

export const useEventEditor = create<EventEditorState>((set) => ({
  open: false,
  editing: null,
  day: null,
  close: () => set({ open: false, editing: null, day: null }),
}));

/** Open the dialog on a blank event, starting on `day` (default today). */
export function newEvent(day?: Date) {
  useEventEditor.setState({ open: true, editing: null, day: day ?? null });
}

/**
 * Open the dialog on an existing local event.
 *
 * Only a local row can be edited — a Canvas row would be overwritten by the
 * next sync and a task belongs to its board — so callers gate on
 * `event.localId != null`, which is the same test that gates Remove.
 */
export function editEvent(event: CalEvent) {
  useEventEditor.setState({ open: true, editing: event, day: null });
}

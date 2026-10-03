import { create } from "zustand";
import type { CalEvent } from "@/lib/calendar";

/** The new/edit event dialog, a store because its triggers live in different
 *  subtrees; one dialog in `AppLayout` answers. */
interface EventEditorState {
  open: boolean;
  /** Each opening owns its async save completion. */
  session: number;
  /** `null` while creating a new one. */
  editing: CalEvent | null;
  /** The day a new event starts on — the calendar's anchor, not today. */
  day: Date | null;
  close: () => void;
  closeSaved: (session: number) => void;
}

export const useEventEditor = create<EventEditorState>((set) => ({
  open: false,
  session: 0,
  editing: null,
  day: null,
  close: () => set({ open: false, editing: null, day: null }),
  closeSaved: (session) => set((state) =>
    state.open && state.session === session
      ? { open: false, editing: null, day: null }
      : state,
  ),
}));

/** Open the dialog on a blank event, starting on `day` (default today). */
export function newEvent(day?: Date) {
  useEventEditor.setState((state) => ({
    open: true, session: state.session + 1, editing: null, day: day ?? null,
  }));
}

/** Open the dialog on a local event; callers gate on `event.localId != null`,
 *  since synced rows would be overwritten and tasks belong to their board. */
export function editEvent(event: CalEvent) {
  useEventEditor.setState((state) => ({
    open: true, session: state.session + 1, editing: event, day: null,
  }));
}

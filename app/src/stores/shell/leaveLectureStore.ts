import { create } from "zustand";
import { isLecturePlaying, playingLecture } from "@/lib/lectures/playback";

interface Pending {
  title: string;
  leave: () => void;
  stay: () => void;
}

interface LeaveLectureState {
  pending: Pending | null;
  confirm: () => void;
  cancel: () => void;
}

/** The "leave your lecture?" prompt: the player and the tab strip both ask
 *  here, and one dialog in `AppLayout` answers. */
export const useLeaveLecture = create<LeaveLectureState>((set, get) => ({
  pending: null,
  confirm: () => {
    const p = get().pending;
    set({ pending: null });
    p?.leave();
  },
  cancel: () => {
    const p = get().pending;
    set({ pending: null });
    p?.stay();
  },
}));

/** Run `leave`, asking first if a lecture is playing. `stay` undoes whatever
 *  the caller started before asking (e.g. releasing a router blocker). */
export function confirmLeavingLecture(leave: () => void, stay = () => {}) {
  const lecture = playingLecture();
  if (!isLecturePlaying() || !lecture) {
    leave();
    return;
  }
  useLeaveLecture.setState({ pending: { title: lecture.title, leave, stay } });
}

import { create } from "zustand";
import { isLecturePlaying, playingLecture } from "@/lib/lecturePlayback";

/**
 * The "leave your lecture?" prompt, as a store rather than local state,
 * because the two things that can strand a playing lecture are in different
 * corners of the tree: the player itself (a navigation out of its tab, caught
 * by the router blocker) and the tab strip (closing its tab). Both ask through
 * here and one dialog in `AppLayout` answers.
 */
interface Pending {
  /** What the lecture is called, so the dialog can name it. */
  title: string;
  leave: () => void;
  stay: () => void;
}

interface LeaveLectureState {
  pending: Pending | null;
  confirm: () => void;
  cancel: () => void;
}

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

/**
 * Run `leave` — after asking, if a lecture is playing and would be left behind.
 * `stay` is the undo for whatever the caller had to start before asking (the
 * router's blocker, most of all, which has to be released either way).
 */
export function confirmLeavingLecture(leave: () => void, stay = () => {}) {
  const lecture = playingLecture();
  if (!isLecturePlaying() || !lecture) {
    leave();
    return;
  }
  useLeaveLecture.setState({ pending: { title: lecture.title, leave, stay } });
}

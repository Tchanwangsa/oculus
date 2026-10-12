import { create } from "zustand";

import { listVideos, type VideoRow } from "@/lib/studio/videos";

interface VideosState {
  /** Newest first. */
  videos: VideoRow[];
  loaded: boolean;
  /** The last read's failure, shown in place of the list. */
  error: string | null;
  reload: () => Promise<void>;
}

/** Latest read wins, so a slow answer never overwrites a newer one. */
let run = 0;

/** Every video, for the Recent list and the Videos grid. The grid filters
 *  by subject itself. */
export const useVideosStore = create<VideosState>((set) => ({
  videos: [],
  loaded: false,
  error: null,
  reload: async () => {
    const mine = ++run;
    try {
      const videos = await listVideos();
      if (mine === run) set({ videos, loaded: true, error: null });
    } catch (e) {
      if (mine === run) set({ loaded: true, error: e instanceof Error ? e.message : String(e) });
    }
  },
}));

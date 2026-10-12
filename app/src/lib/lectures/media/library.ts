/** Library videos' own elements (`VideoFileViewer`), so a lecture or music
 *  starting can silence them; the lecture's are `lib/lectures/playback/`'s. */
const libraryVideos = new Set<HTMLVideoElement>();
/** Told whenever a library video plays or pauses (`lib/music/exclusive.ts`). */
const listeners = new Set<() => void>();

const notify = () => {
  for (const fn of listeners) fn();
};

export function registerLibraryVideo(v: HTMLVideoElement): () => void {
  libraryVideos.add(v);
  v.addEventListener("play", notify);
  v.addEventListener("pause", notify);
  return () => {
    v.removeEventListener("play", notify);
    v.removeEventListener("pause", notify);
    libraryVideos.delete(v);
    notify();
  };
}

/** Pause every library video but `except`. */
export function pauseLibraryVideos(except?: HTMLVideoElement) {
  for (const v of libraryVideos) if (v !== except && !v.paused) v.pause();
}

export function isLibraryVideoPlaying(): boolean {
  for (const v of libraryVideos) if (!v.paused) return true;
  return false;
}

export function subscribeLibraryVideos(fn: () => void): () => void {
  listeners.add(fn);
  return () => void listeners.delete(fn);
}

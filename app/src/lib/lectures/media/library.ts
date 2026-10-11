/** Library videos' own elements (`VideoFileViewer`), so a lecture starting
 *  can silence them; the lecture's are `lib/lectures/playback/`'s. */
const libraryVideos = new Set<HTMLVideoElement>();

export function registerLibraryVideo(v: HTMLVideoElement): () => void {
  libraryVideos.add(v);
  return () => void libraryVideos.delete(v);
}

/** Pause every library video but `except`. */
export function pauseLibraryVideos(except?: HTMLVideoElement) {
  for (const v of libraryVideos) if (v !== except && !v.paused) v.pause();
}

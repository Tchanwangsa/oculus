import { useCallback, useEffect, useState } from "react";
import { getLecture, getLectureByFolder } from "@/lib/db";

/** Lecture titles by recording folder, read once per folder for the app's
 *  life: a row names its lecture as the lecture page does. */
const titles = new Map<string, string | null>();

export function useLectureTitles(folders: string[]): (folder: string) => string | undefined {
  const [, bump] = useState(0);
  const key = folders.join("\n");
  useEffect(() => {
    const missing = key.split("\n").filter((folder) => folder && !titles.has(folder));
    if (missing.length === 0) return;
    let live = true;
    Promise.all(
      missing.map((folder) =>
        getLectureByFolder(folder)
          .then((l) => titles.set(folder, l?.title ?? null))
          .catch((e) => {
            console.error("thread info: lecture title", e);
            titles.set(folder, null);
          }),
      ),
    ).then(() => live && bump((n) => n + 1));
    return () => {
      live = false;
    };
  }, [key]);
  return useCallback((folder: string) => titles.get(folder) ?? undefined, []);
}

/** A lecture thread's recording folder (`lectures.folder_rel`): what its
 *  reads are keyed by. Null until the row is read, and for a lecture with
 *  nothing downloaded. */
export function useLectureFolder(lectureId: string | null): string | null {
  const [folder, setFolder] = useState<{ id: string; folder: string | null } | null>(null);
  useEffect(() => {
    if (!lectureId) return;
    let live = true;
    getLecture(lectureId)
      .then((l) => live && setFolder({ id: lectureId, folder: l?.folder_rel ?? null }))
      .catch((e) => console.error("thread info: lecture folder", e));
    return () => {
      live = false;
    };
  }, [lectureId]);
  return folder?.id === lectureId ? folder.folder : null;
}

import { useCallback, useEffect, useState } from "react";
import { getLecture } from "@/lib/db";

/** Lecture titles by id, read once per id for the app's life: a row names
 *  its lecture as the lecture page does. */
const titles = new Map<string, string | null>();

export function useLectureTitles(ids: string[]): (id: string) => string | undefined {
  const [, bump] = useState(0);
  const key = ids.join("\n");
  useEffect(() => {
    const missing = key.split("\n").filter((id) => id && !titles.has(id));
    if (missing.length === 0) return;
    let live = true;
    Promise.all(
      missing.map((id) =>
        getLecture(id)
          .then((l) => titles.set(id, l?.title ?? null))
          .catch((e) => {
            console.error("thread info: lecture title", e);
            titles.set(id, null);
          }),
      ),
    ).then(() => live && bump((n) => n + 1));
    return () => {
      live = false;
    };
  }, [key]);
  return useCallback((id: string) => titles.get(id) ?? undefined, []);
}

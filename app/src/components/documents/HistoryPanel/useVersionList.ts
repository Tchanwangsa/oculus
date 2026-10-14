import { useCallback, useEffect, useRef, useState } from "react";

import { useWindowEvent } from "@/hooks/backend/useEvents";
import { DOCUMENT_VERSIONS_EVENT, listVersions, type DocumentVersion } from "@/lib/notes/documentVersions";

/** The note's versions, newest first, reread when its versions change, and
 *  which row is selected. A row is always selected once the list has any:
 *  the newest at first, and after a delete the row that took its place. */
export function useVersionList(fileId: number) {
  /** Null while the first read is out. */
  const [versions, setVersions] = useState<DocumentVersion[] | null>(null);
  const [listError, setListError] = useState<string | null>(null);
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const selected = versions?.find((v) => v.id === selectedId) ?? null;

  // Only the newest read lands, so a burst of events can't paint an old list.
  const readSeq = useRef(0);
  const load = useCallback(() => {
    const seq = ++readSeq.current;
    listVersions(fileId)
      .then((rows) => {
        if (seq !== readSeq.current) return;
        setVersions(rows);
        setListError(null);
      })
      .catch((e) => seq === readSeq.current && setListError(String(e)));
  }, [fileId]);
  useEffect(() => {
    load();
    return () => {
      readSeq.current++;
    };
  }, [load]);
  useWindowEvent(DOCUMENT_VERSIONS_EVENT, (e) => {
    const id = (e as CustomEvent<{ fileId?: number }>).detail?.fileId;
    if (id == null || id === fileId) load();
  });

  // The selected row's last index, so a deleted row hands over to its neighbour.
  const lastIndex = useRef(0);
  useEffect(() => {
    if (!versions?.length) return;
    const i = versions.findIndex((v) => v.id === selectedId);
    if (i >= 0) lastIndex.current = i;
    else setSelectedId(versions[Math.min(lastIndex.current, versions.length - 1)].id);
  }, [versions, selectedId]);

  return { versions, listError, selectedId, setSelectedId, selected };
}

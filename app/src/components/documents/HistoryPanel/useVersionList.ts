import { useCallback, useEffect, useRef, useState } from "react";

import { useWindowEvent } from "@/hooks/backend/useEvents";
import { DOCUMENT_VERSIONS_EVENT, listVersions, type DocumentVersion } from "@/lib/notes/documentVersions";

/** The note's versions, newest first, reread when its versions change, and
 *  which row is selected. */
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

  return { versions, listError, selectedId, setSelectedId, selected };
}

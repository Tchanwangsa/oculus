import { useCallback, useRef } from "react";

import { versionText, type DocumentVersion } from "@/lib/notes/documentVersions";

/** Versions' texts, each read once (a version never changes), for the page
 *  and the restore actions alike. */
export function useVersionTexts() {
  const texts = useRef(new Map<number, string>());
  const textOf = useCallback(async (v: DocumentVersion): Promise<string> => {
    const cached = texts.current.get(v.id);
    if (cached != null) return cached;
    const text = await versionText(v.id);
    texts.current.set(v.id, text);
    return text;
  }, []);
  return { texts, textOf };
}

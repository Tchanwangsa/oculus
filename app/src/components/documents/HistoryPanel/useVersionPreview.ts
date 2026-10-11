import { useEffect, useRef, useState } from "react";

import { versionText, type DocumentVersion } from "@/lib/notes/documentVersions";

/** The selected version's text, read once per version. `textOf` serves the
 *  restore actions from the same cache. */
export function useVersionPreview(selectedId: number | null) {
  // A version's text never changes, so each is read once.
  const texts = useRef(new Map<number, string>());
  const [preview, setPreview] = useState<{ id: number; text: string } | null>(null);
  const [previewError, setPreviewError] = useState<string | null>(null);
  useEffect(() => {
    setPreviewError(null);
    if (selectedId == null) return;
    const cached = texts.current.get(selectedId);
    if (cached != null) {
      setPreview({ id: selectedId, text: cached });
      return;
    }
    let live = true;
    versionText(selectedId)
      .then((text) => {
        texts.current.set(selectedId, text);
        if (live) setPreview({ id: selectedId, text });
      })
      .catch((e) => live && setPreviewError(String(e)));
    return () => {
      live = false;
    };
  }, [selectedId]);
  const textOf = async (v: DocumentVersion): Promise<string> => {
    const cached = texts.current.get(v.id);
    if (cached != null) return cached;
    const text = await versionText(v.id);
    texts.current.set(v.id, text);
    return text;
  };

  return { texts, preview, previewError, textOf };
}

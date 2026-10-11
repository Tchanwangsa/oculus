import { useMemo, useRef, type RefObject } from "react";

import { cancelDocumentSuggestion, suggestDocument } from "@/lib/notes/documents";
import type { DbFile } from "@/lib/db";

import type { SuggestConfig } from "../editor/chrome/aiSuggest";
import type { SuggestStatus } from "../DocumentControls";

/** The inline AI suggestion plugin's config (`../editor/chrome/aiSuggest.ts`),
 *  and the latest toggle for a view made after it. */
export function useNoteSuggest(
  fileRef: RefObject<DbFile>,
  suggestions: boolean,
  onSuggestStatus: (status: SuggestStatus) => void,
) {
  const suggestionsRef = useRef(suggestions);
  suggestionsRef.current = suggestions;
  const suggestStatusRef = useRef(onSuggestStatus);
  suggestStatusRef.current = onSuggestStatus;

  /** Stable, so the toggle only swaps the compartment; the path is read per
   *  request, so a rename needs nothing. */
  const suggestConfig = useMemo<SuggestConfig>(
    () => ({
      fetch: ({ requestId, before, after }) =>
        suggestDocument({ requestId, path: fileRef.current.relative_path, before, after }),
      cancel: () => void cancelDocumentSuggestion().catch(console.error),
      onStatus: (s) => suggestStatusRef.current(s),
    }),
    [fileRef],
  );

  return { suggestionsRef, suggestConfig };
}

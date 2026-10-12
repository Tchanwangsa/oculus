import { createContext, useContext, useMemo, type ReactNode } from "react";
import { useSubjects } from "@/hooks/data/useSubjects";

/**
 * The subject whose library file is on screen, for the commands of the
 * Echo360 and Canvas video cards. `"loading"` until the subjects list
 * answers; null without a provider or for a subject no longer listed, where
 * those cards are only a link out.
 */
export type EmbedSubject = { subjectId: number; subjectCode: string } | "loading" | null;

const EmbedSubjectContext = createContext<EmbedSubject>(null);

export const useEmbedSubject = () => useContext(EmbedSubjectContext);

/** The code comes from the subjects list by id: a path's course folder is
 *  `safe_dir(code)`, not necessarily the code. */
export function EmbedSubjectProvider({ subjectId, children }: { subjectId: number; children: ReactNode }) {
  const { subjects, loading } = useSubjects();
  const code = subjects.find((s) => s.id === subjectId)?.code ?? null;
  const value = useMemo<EmbedSubject>(
    () => (code ? { subjectId, subjectCode: code } : loading ? "loading" : null),
    [subjectId, code, loading],
  );
  return <EmbedSubjectContext.Provider value={value}>{children}</EmbedSubjectContext.Provider>;
}

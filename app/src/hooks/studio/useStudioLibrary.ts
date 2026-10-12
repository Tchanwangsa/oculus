import { useEffect, useMemo } from "react";

import { useStoredState } from "@/hooks/ui/useStoredState";
import { useHarnessStore } from "@/stores/chat/harnessStore";

const readStored = (raw: string | null) => raw ?? "";

/**
 * Whose library the Components and Themes pages browse: the global one, or a
 * subject's by course code. One remembered choice serves both pages, so
 * switching on one shows the same library on the other.
 */
export function useStudioLibrary() {
  const subjects = useHarnessStore((s) => s.subjects);
  const [stored, setStored] = useStoredState("oculus-studio-components-library", readStored);

  useEffect(() => {
    void useHarnessStore.getState().loadSubjects();
  }, []);

  // A remembered subject that is gone shows the global library.
  const subject = useMemo(() => subjects.find((s) => s.code === stored) ?? null, [subjects, stored]);
  // Until the subjects arrive the remembered code is taken as it is.
  const code = subject ? subject.code : subjects.length === 0 && stored ? stored : null;
  return {
    subjects,
    /** The subject row, once the subjects are loaded. */
    subject,
    /** The library's course code; null is the global library. */
    code,
    setCode: (next: string | null) => setStored(next ?? ""),
  };
}

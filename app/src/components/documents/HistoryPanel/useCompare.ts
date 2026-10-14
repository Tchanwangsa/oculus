import { useStoredState } from "@/hooks/ui/useStoredState";

/** What a version is marked against: the next-older version, or the note now. */
export type CompareWith = "previous" | "current";

const readCompare = (raw: string | null): CompareWith => (raw === "current" ? "current" : "previous");
const readShow = (raw: string | null) => raw !== "0";
const writeShow = (on: boolean) => (on ? "1" : "0");

/** The version bar's compare choice and "Show changes", kept across notes. */
export function useCompare() {
  const [compare, setCompare] = useStoredState<CompareWith>("oculus-history-compare", readCompare);
  const [showChanges, setShowChanges] = useStoredState("oculus-history-show-changes", readShow, writeShow);
  return { compare, setCompare, showChanges, setShowChanges };
}

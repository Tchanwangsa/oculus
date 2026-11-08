import { lazy, Suspense } from "react";
import { CompactMd } from "@/components/markdown/MdComponents";
import { useSubjects } from "@/hooks/data/useSubjects";

const MarkdownField = lazy(() => import("@/components/revision/MarkdownField").then((module) => ({ default: module.MarkdownField })));
const committed = () => {};

/** Proposal drafts update on every keystroke, before Apply or Cancel can run.
 * A blur/unmount must not commit an older draft over a cancelled edit. */
export function MarkdownInput({ value, label, subject, subjectId, onChange }: {
  value: unknown; label: string; subject?: unknown; subjectId?: number | null; onChange: (text: string) => void;
}) {
  const { subjects } = useSubjects();
  const scope = subjectId ?? subjects.find((row) => row.code === subject)?.id ?? null;
  const text = String(value ?? "");
  return <Suspense fallback={<CompactMd text={text} />}>
    <MarkdownField compact controlled text={text} label={label} placeholder={`Write ${label.toLowerCase()}…`} subjectId={scope} onChange={onChange} onCommit={committed} />
  </Suspense>;
}

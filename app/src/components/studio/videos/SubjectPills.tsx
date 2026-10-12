import { PillTabs } from "@/components/ui/table/PillTabs";
import { displayCode } from "@/lib/format/format";

export const ALL = "all";

/** Pills over the grid: All, then each subject that has a video. Hidden when
 *  there is only one thing to choose. */
export function SubjectPills({
  subjects,
  value,
  onChange,
}: {
  /** Course codes, in the order to show them. */
  subjects: string[];
  value: string;
  onChange: (subject: string) => void;
}) {
  if (subjects.length < 2) return null;
  return (
    <PillTabs
      tabs={[{ value: ALL, label: "All" }, ...subjects.map((s) => ({ value: s, label: displayCode(s) }))]}
      value={value}
      onChange={onChange}
      className="flex-wrap gap-1"
    />
  );
}

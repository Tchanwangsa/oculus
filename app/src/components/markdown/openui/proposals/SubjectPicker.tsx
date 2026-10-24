import type { ReactNode } from "react";
import { FilterPopover, FilterRow } from "@/components/ui/filter/FilterPopover";
import { FilterList } from "@/components/ui/filter/FilterList";
import { useSubjects } from "@/hooks/data/useSubjects";
import { displayCode } from "@/lib/format/format";

/**
 * A proposal's subject as a popover of the student's subjects, each named by
 * its short code and term. `none` adds a row that clears it (a project's
 * Personal); a code the student has no subject for stays listed as chosen.
 */
export function SubjectPicker({ value, none, icon, triggerClassName, onChange }: {
  value: string | null; none?: string; icon?: ReactNode; triggerClassName?: string; onChange: (code: string | null) => void;
}) {
  const { subjects } = useSubjects();
  const known = subjects.find((s) => s.code === value);
  return <FilterPopover label={value ? displayCode(value) : none ?? "Subject"} active={false} icon={icon} className="w-64" triggerClassName={triggerClassName}>{(close) => <FilterList>
    {none && <FilterRow label={none} selected={value == null} onPick={() => { onChange(null); close(); }} />}
    {value && !known && <FilterRow label={displayCode(value)} hint={value} selected onPick={close} />}
    {subjects.map((s) => <FilterRow key={s.id} label={`${displayCode(s.code)}${s.term_name ? ` · ${s.term_name}` : ""}`} hint={s.name} selected={s.code === value} onPick={() => { onChange(s.code); close(); }} />)}
  </FilterList>}</FilterPopover>;
}

import { GraduationCap, User } from "@phosphor-icons/react";
import { FilterFold } from "@/components/ui/filter/FilterFold";
import { FilterList } from "@/components/ui/filter/FilterList";
import { FilterPopover, FilterRow } from "@/components/ui/filter/FilterPopover";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import { useSubjects } from "@/hooks/data/useSubjects";
import { displayCode } from "@/lib/format/format";
import { TOOLBAR_PILL } from "./toolbarPill";
import { pickSubjectFilter, type TaskFilterCounts } from "./filterCounts";
import type { TaskFilter } from "./filterRules";

/**
 * The Subject pill: Any, Personal (no subject — personal projects and anything
 * unfiled) or one subject. Past-term subjects sit in a fold, initially open when one of
 * them is picked. Picking closes the popover.
 */
export function SubjectFilter({
  filter,
  counts,
  onChange,
}: {
  filter: TaskFilter;
  counts: TaskFilterCounts;
  onChange: (next: TaskFilter) => void;
}) {
  const { current, past } = useSubjects();
  const value = filter.subject;
  const label = value === "any" ? "Subject" : value === "personal" ? "Personal" : displayCode(value);

  return (
    <FilterPopover
      label={label}
      active={value !== "any"}
      icon={
        value === "personal" ? (
          <User size={13} />
        ) : value !== "any" ? (
          <SubjectIcon code={value} size={13} />
        ) : (
          <GraduationCap size={13} className={value === "any" ? "text-muted-foreground" : undefined} />
        )
      }
      className="w-64"
      triggerClassName={TOOLBAR_PILL}
    >
      {(close) => {
        const pick = (subject: TaskFilter["subject"]) => {
          onChange(pickSubjectFilter(filter, subject));
          close();
        };
        const row = (code: string) => (
          <FilterRow
            key={code}
            label={displayCode(code)}
            count={counts.subjects.get(code) ?? 0}
            icon={<SubjectIcon code={code} size={13} />}
            selected={value === code}
            onPick={() => pick(code)}
          />
        );
        return (
          <FilterList>
            <FilterRow count={counts.anySubject} label="Any subject" selected={value === "any"} onPick={() => pick("any")} />
            <FilterRow
              label="Personal"
              count={counts.subjects.get(null) ?? 0}
              icon={<User size={13} className="shrink-0 text-muted-foreground" />}
              hint="No subject — personal projects, and anything unfiled"
              selected={value === "personal"}
              onPick={() => pick("personal")}
            />
            {current.map((s) => row(s.code))}
            {past.length > 0 && (
              <FilterFold label="Past subjects" count={past.length} defaultOpen={past.some((s) => s.code === value)}>
                {past.map((s) => row(s.code))}
              </FilterFold>
            )}
          </FilterList>
        );
      }}
    </FilterPopover>
  );
}

import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import { Select, SelectContent, SelectItem, SelectSeparator, SelectTrigger, SelectValue } from "@/components/ui/select";
import type { Subject } from "@/lib/db";
import { displayCode, displayName } from "@/lib/format/format";

/** Radix rejects an empty-string value. */
const GLOBAL = "global";

/**
 * Whose library the page shows: the global one, or a subject's, whose
 * components replace the global ones of the same name. Holds a course code,
 * the name of the subject's folder.
 */
export function LibrarySelect({
  subjects,
  value,
  onChange,
}: {
  subjects: Subject[];
  /** A course code, or null for the global library. */
  value: string | null;
  onChange: (code: string | null) => void;
}) {
  const active = subjects.find((s) => s.code === value) ?? null;
  // This term's subjects, plus the chosen one so a past choice still shows.
  const listed = subjects.filter((s) => s.is_current || s.code === value);
  return (
    <Select value={value ?? GLOBAL} onValueChange={(v) => onChange(v === GLOBAL ? null : v)}>
      <SelectTrigger
        size="sm"
        aria-label="Library"
        title={active ? displayName(active.name, active.code) : "The global library"}
        className="h-7 gap-1.5 text-xs"
      >
        <SelectValue>
          {active ? (
            <span className="flex items-center gap-1.5">
              <SubjectIcon code={active.code} size={12} />
              {displayCode(active.code)}
            </span>
          ) : (
            "Global"
          )}
        </SelectValue>
      </SelectTrigger>
      <SelectContent align="end" className="max-h-56 min-w-[9rem]">
        <SelectItem value={GLOBAL} className="text-xs">
          Global
        </SelectItem>
        {listed.length > 0 && <SelectSeparator />}
        {listed.map((s) => (
          <SelectItem key={s.id} value={s.code} title={displayName(s.name, s.code)} className="gap-1.5 text-xs">
            <SubjectIcon code={s.code} size={12} />
            <span>{displayCode(s.code)}</span>
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}

import {
  Select,
  SelectContent,
  SelectItem,
  SelectSeparator,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import { displayCode, displayName } from "@/lib/format";
import type { Subject } from "@/lib/db";
import { cn } from "@/lib/utils";

/** Radix rejects an empty-string value, so "no subject" needs a name. */
const GENERAL = "general";

/**
 * A thread's scope: one subject, or General (the whole library). Not a sandbox
 * — it narrows the `@` menu and names the course folder in the agent's
 * instructions (`docs/harness.md`). Fixed once the thread starts, since the
 * CLIs bind instructions at session start.
 */
export function SubjectSelect({
  subjects,
  value,
  onChange,
  className,
  disabled,
}: {
  subjects: Subject[];
  value: number | null;
  onChange: (subjectId: number | null) => void;
  className?: string;
  disabled?: boolean;
}) {
  const active = subjects.find((s) => s.id === value) ?? null;
  // This term's subjects, plus the current value so a past scope still shows.
  const listed = subjects.filter((s) => s.is_current || s.id === value);
  return (
    <Select
      value={value == null ? GENERAL : String(value)}
      onValueChange={(v) => onChange(v === GENERAL ? null : Number(v))}
      disabled={disabled}
    >
      <SelectTrigger
        size="sm"
        aria-label="Subject"
        title={active ? displayName(active.name, active.code) : "Every subject"}
        className={cn("h-7 gap-1.5 text-xs", className)}
      >
        <SelectValue>
          {active ? (
            <span className="flex items-center gap-1.5">
              <SubjectIcon code={active.code} size={12} />
              {displayCode(active.code)}
            </span>
          ) : (
            "General"
          )}
        </SelectValue>
      </SelectTrigger>
      <SelectContent className="max-h-56 min-w-[9rem]">
        <SelectItem value={GENERAL} className="text-xs">
          General
        </SelectItem>
        {listed.length > 0 && <SelectSeparator />}
        {listed.map((s) => (
          <SelectItem
            key={s.id}
            value={String(s.id)}
            title={displayName(s.name, s.code)}
            className="gap-1.5 text-xs"
          >
            <SubjectIcon code={s.code} size={12} />
            <span>{displayCode(s.code)}</span>
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}

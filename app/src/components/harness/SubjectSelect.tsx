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

/** Radix will not hold an empty string as a value, and "no subject" is a real
 *  choice rather than an unset one, so it gets a name of its own. */
const GENERAL = "general";

/**
 * What the thread is about: one subject, or the whole library.
 *
 * The scope is not a sandbox — every thread runs from `agents/` and can read
 * all of `../courses/` either way (`docs/harness.md`). What it changes is
 * where the agent is pointed: a subject narrows the composer's `@` menu to
 * that subject's files and names the course folder in the appended
 * instructions, so "what's due this week" means that subject. General is the
 * library-wide thread, where an answer has to say which subject it came from.
 *
 * A thread keeps the scope it was opened with, the same way it keeps its
 * provider: both CLIs bind the instructions at session start, so a change
 * mid-thread would be a lie until the process was restarted.
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
  // Only this term's subjects — a new thread is about what is being studied
  // now, and the full list is long enough to bury it. A thread already scoped
  // to a past subject still shows its own row, or the trigger would read as
  // General.
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
        {/* Icon plus code, the way the sidebar names a subject; the full name
            is the tooltip on both the trigger and the rows. */}
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

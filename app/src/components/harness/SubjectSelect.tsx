import {
  Select,
  SelectContent,
  SelectItem,
  SelectSeparator,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
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
        className={cn("h-7 text-xs", className)}
      >
        {/* The rows carry the code *and* the full name so the menu can be
            read; the trigger takes the code alone, the way the model picker
            beside it takes the model's name and drops the vendor. */}
        <SelectValue>{active ? displayCode(active.code) : "General"}</SelectValue>
      </SelectTrigger>
      <SelectContent className="max-h-72">
        <SelectItem value={GENERAL} className="text-xs">
          General
        </SelectItem>
        {subjects.length > 0 && <SelectSeparator />}
        {subjects.map((s) => (
          <SelectItem key={s.id} value={String(s.id)} className="text-xs">
            <span>{displayCode(s.code)}</span>
            <span className="truncate text-muted-foreground/70">
              {displayName(s.name, s.code)}
            </span>
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}

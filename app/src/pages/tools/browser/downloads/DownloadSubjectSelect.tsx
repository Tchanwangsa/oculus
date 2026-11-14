import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import { displayCode, displayName } from "@/lib/format/format";
import type { Subject } from "@/lib/db";
import { cn } from "@/lib/utils";

/**
 * The subject a download is saved to, by course code: the composer's
 * `SubjectSelect` without its General entry, since an Oculus save always
 * names a subject. Lists this term's subjects plus the chosen one, or every
 * subject when none is current.
 */
export function DownloadSubjectSelect({
  subjects,
  value,
  onChange,
  disabled,
  className,
}: {
  subjects: Subject[];
  value: string | null;
  onChange: (code: string) => void;
  disabled?: boolean;
  className?: string;
}) {
  const active = subjects.find((s) => s.code === value) ?? null;
  const current = subjects.filter((s) => s.is_current || s.code === value);
  const listed = current.some((s) => s.is_current) ? current : subjects;
  return (
    // An empty value shows the placeholder.
    <Select value={value ?? ""} onValueChange={onChange} disabled={disabled}>
      <SelectTrigger
        size="sm"
        aria-label="Subject"
        title={active ? displayName(active.name, active.code) : undefined}
        className={cn("h-7 gap-1.5 text-xs", className)}
      >
        <SelectValue placeholder="Choose a subject">
          {active ? (
            <span className="flex items-center gap-1.5">
              <SubjectIcon code={active.code} size={12} />
              {displayCode(active.code)}
            </span>
          ) : undefined}
        </SelectValue>
      </SelectTrigger>
      <SelectContent align="end" className="max-h-56 min-w-[9rem]">
        {listed.map((s) => (
          <SelectItem
            key={s.id}
            value={s.code}
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

import { SubjectIcon } from "@/components/subjects/SubjectIcon";
import { TOPIC } from "@/components/revision/cheat/CheatEntry";
import { displayCode } from "@/lib/format/format";
import { cn } from "@/lib/utils";

/** A subject as the rest of the app names it: its icon and short code, in
 * the cheat sheet's chip; the full Canvas code is the tooltip. */
export function SubjectLabel({ code, className }: { code: string; className?: string }) {
  return <span title={code} className={cn(TOPIC, "max-w-full min-w-0 shrink-0 gap-1 text-muted-foreground", className)}>
    <SubjectIcon code={code} size={11} className="shrink-0" />
    <span className="min-w-0 truncate">{displayCode(code)}</span>
  </span>;
}

/** A neutral chip in the same shape, for a file, a lecture or a count. */
export const CHIP = cn(TOPIC, "max-w-full min-w-0 gap-1 text-muted-foreground");

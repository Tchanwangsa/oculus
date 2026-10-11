import { useState } from "react";
import { CaretRight, CircleNotch } from "@phosphor-icons/react";
import { SubjectRow } from "@/components/subjects/SubjectRow";
import type { Subject } from "@/lib/db";
import { cn } from "@/lib/utils";

/**
 * Which subjects the first sync fetches: this term's as a two-column grid of
 * the Sync page's `SubjectRow`s, past terms folded under a toggle. A fetch
 * still running shows one line; its failure shows Rust's sentence.
 */
export function SubjectChecklist({
  subjects,
  fetching,
  error,
  onToggle,
}: {
  subjects: Subject[];
  fetching: boolean;
  error: string | null;
  onToggle: (id: number) => void;
}) {
  const [showPast, setShowPast] = useState(false);
  const current = subjects.filter((s) => s.is_current);
  const past = subjects.filter((s) => !s.is_current);
  const shown = showPast ? [...current, ...past] : current;

  return (
    <section className="flex flex-col gap-2.5">
      <div className="flex items-baseline justify-between gap-3">
        <h2 className="text-[13px] font-medium text-foreground">Subjects to sync</h2>
        {current[0]?.term_name && (
          <span className="text-xs text-muted-foreground">{current[0].term_name}</span>
        )}
      </div>

      {shown.length > 0 && (
        <div className="grid grid-cols-2 gap-1.5">
          {shown.map((s) => (
            <SubjectRow
              key={s.id}
              subject={s}
              checked={s.selected}
              onToggle={() => onToggle(s.id)}
              dimmed={!s.is_current}
            />
          ))}
        </div>
      )}

      {fetching && (
        <p className="flex items-center gap-2 text-xs text-muted-foreground">
          <CircleNotch size={13} className="animate-spin" />
          {subjects.length === 0 ? "Fetching your subjects from Canvas…" : "Checking Canvas for changes…"}
        </p>
      )}

      {!fetching && !error && subjects.length > 0 && current.length === 0 && !showPast && (
        <p className="text-xs text-muted-foreground">Canvas lists no subjects for this term.</p>
      )}

      {!fetching && !error && subjects.length === 0 && (
        <p className="text-xs text-muted-foreground">Canvas didn't list any subjects.</p>
      )}

      {error && (
        <p data-selectable className="text-xs/relaxed text-destructive">
          Couldn't fetch your subjects: {error}
        </p>
      )}

      {past.length > 0 && (
        <button
          type="button"
          onClick={() => setShowPast((v) => !v)}
          className="flex w-fit cursor-pointer items-center gap-1.5 text-xs text-muted-foreground transition-colors hover:text-foreground"
        >
          <CaretRight size={10} className={cn("shrink-0", showPast && "rotate-90")} />
          {showPast ? "Hide past subjects" : `Past subjects (${past.length})`}
        </button>
      )}
    </section>
  );
}

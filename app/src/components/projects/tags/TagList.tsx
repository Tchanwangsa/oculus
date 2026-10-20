import { cn } from "@/lib/utils";
import type { DbTaskTag } from "@/lib/planning/projects";
import { TagChip } from "./TagChip";

/**
 * A task's tags as chips on one line. Past `max` the rest collapse to `+N`;
 * a chip truncates before the row wraps. An id the registry no longer holds
 * (a tag deleted mid-read) is skipped.
 */
export function TagList({
  tagIds,
  byId,
  max,
  className,
}: {
  tagIds: number[];
  byId: Map<number, DbTaskTag>;
  max?: number;
  className?: string;
}) {
  const tags = tagIds.flatMap((id) => byId.get(id) ?? []);
  if (!tags.length) return null;
  const shown = max === undefined ? tags : tags.slice(0, max);
  const hidden = tags.length - shown.length;
  return (
    <span className={cn("flex min-w-0 flex-nowrap items-center gap-1 overflow-hidden", className)}>
      {shown.map((tag) => (
        <TagChip key={tag.id} tag={tag} className="shrink" />
      ))}
      {hidden > 0 && (
        <span
          title={tags
            .slice(shown.length)
            .map((t) => t.name)
            .join(", ")}
          className="shrink-0 text-[11px] text-muted-foreground tabular-nums"
        >
          +{hidden}
        </span>
      )}
    </span>
  );
}

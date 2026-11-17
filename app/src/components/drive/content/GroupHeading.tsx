import { CaretDown, CaretRight } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { isFolded, type DriveGroup, type GroupFolds } from "./groups";

/** A group's heading: a plain label, or for a foldable group a button that
 *  folds it, with its count while folded. The caret swaps icons rather than
 *  rotating, so nothing transitions on the icon. */
export function GroupHeading({ group, folds, className }: { group: DriveGroup; folds: GroupFolds; className?: string }) {
  if (!group.label) return null;
  const label = <span className="text-[11px] font-medium text-muted-foreground">{group.label}</span>;
  if (!group.foldable) return <div className={cn("flex items-center", className)}>{label}</div>;
  const folded = isFolded(group, folds);
  const Caret = folded ? CaretRight : CaretDown;
  return (
    <button
      type="button"
      aria-expanded={!folded}
      onClick={() => folds.toggle(group.key)}
      className={cn("flex cursor-pointer items-center gap-1.5 text-left outline-none hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring/50 rounded-sm", className)}
    >
      <Caret size={10} className="shrink-0 text-muted-foreground/60" />
      {label}
      {folded && <span className="text-[11px] text-muted-foreground/70 tabular-nums">{group.items.length}</span>}
    </button>
  );
}

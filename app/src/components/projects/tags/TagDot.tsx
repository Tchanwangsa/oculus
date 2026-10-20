import { cn } from "@/lib/utils";
import type { TagColor } from "@/lib/planning/projects";
import { tagColorClasses } from "./tagColors";

/** A tag's colour as an 8px dot. */
export function TagDot({ color, className }: { color: TagColor; className?: string }) {
  return (
    <span
      aria-hidden
      className={cn("inline-block size-2 shrink-0 rounded-full", tagColorClasses(color).dot, className)}
    />
  );
}

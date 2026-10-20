import { ChipRemove } from "@/components/ui/ChipRemove";
import { cn } from "@/lib/utils";
import type { DbTaskTag } from "@/lib/planning/projects";
import { tagColorClasses } from "./tagColors";
import { TagDot } from "./TagDot";

/**
 * A tag as a compact pill: dot, name, soft tint. With `onRemove` it grows an
 * × button — leave that off inside a row that is itself a button, where a
 * nested button loses its clicks in WebKit.
 */
export function TagChip({
  tag,
  onRemove,
  className,
}: {
  tag: DbTaskTag;
  onRemove?: () => void;
  className?: string;
}) {
  const color = tagColorClasses(tag.color);
  return (
    <span
      className={cn(
        "inline-flex h-5 max-w-full min-w-0 items-center gap-1.5 rounded-full px-2 text-[11px] leading-none font-medium",
        color.soft,
        color.text,
        className,
      )}
    >
      <TagDot color={tag.color} />
      <span className="min-w-0 truncate">{tag.name}</span>
      {onRemove && (
        <ChipRemove
          label={`Remove ${tag.name}`}
          onRemove={onRemove}
          size="sm"
          className="-mr-1"
        />
      )}
    </span>
  );
}

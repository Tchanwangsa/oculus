import { X } from "@phosphor-icons/react";
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
      title={tag.name}
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
        <button
          type="button"
          aria-label={`Remove ${tag.name}`}
          onClick={(e) => {
            e.stopPropagation();
            onRemove();
          }}
          className="-mr-1 flex size-4 shrink-0 cursor-pointer items-center justify-center rounded-full opacity-60 transition-opacity hover:opacity-100"
        >
          <X size={9} weight="bold" />
        </button>
      )}
    </span>
  );
}

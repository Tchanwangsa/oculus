import { cn } from "@/lib/utils";
import { TAG_COLORS, type TagColor } from "@/lib/planning/projects";
import { tagColorClasses, tagColorLabel } from "./tagColors";

/** The tag palette as a row of dot buttons; the chosen one is ringed. */
export function TagColorSwatches({
  value,
  onChange,
  className,
}: {
  value: TagColor;
  onChange: (color: TagColor) => void;
  className?: string;
}) {
  return (
    <div role="radiogroup" aria-label="Tag colour" className={cn("flex items-center gap-0.5", className)}>
      {TAG_COLORS.map((color) => {
        const selected = color === value;
        return (
          <button
            key={color}
            type="button"
            role="radio"
            aria-checked={selected}
            aria-label={tagColorLabel(color)}
            title={tagColorLabel(color)}
            onClick={() => {
              if (!selected) onChange(color);
            }}
            className="flex size-6 shrink-0 cursor-pointer items-center justify-center rounded-full outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
          >
            <span
              className={cn(
                "size-3 rounded-full transition-shadow",
                tagColorClasses(color).dot,
                selected
                  ? "ring-2 ring-foreground/70 ring-offset-2 ring-offset-popover"
                  : "hover:ring-2 hover:ring-foreground/20 hover:ring-offset-2 hover:ring-offset-popover",
              )}
            />
          </button>
        );
      })}
    </div>
  );
}

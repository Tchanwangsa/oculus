import type { ComponentProps, Ref } from "react";
import { BookmarkSimple } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";

/** The pill above the composer that opens the stash: "Stash" and how many
 *  prompts it holds. A Radix trigger, so it passes its props and ref on. */
export function StashBadge({
  count,
  ref,
  className,
  ...props
}: { count: number; ref?: Ref<HTMLButtonElement> } & ComponentProps<"button">) {
  return (
    <button
      ref={ref}
      type="button"
      aria-label={`Stashed prompts: ${count}. Open stash.`}
      className={cn(
        "flex h-6 shrink-0 cursor-pointer items-center gap-1 rounded-full px-2 text-[11px] text-muted-foreground hover:bg-accent hover:text-foreground aria-expanded:bg-accent aria-expanded:text-foreground",
        className,
      )}
      {...props}
    >
      <BookmarkSimple size={12} className="shrink-0" />
      Stash
      <span className="tabular-nums">{count}</span>
    </button>
  );
}

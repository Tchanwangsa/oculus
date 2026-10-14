import { X } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";

const ICON = { sm: 9, md: 11 } as const;

/**
 * The × that unlinks a chip, drawn like the tab strip's close button: muted
 * until hovered, then foreground over a subtle fill. The hover changes colour
 * only, so it never gives the button its own compositing layer (which makes
 * icons step half a pixel at fractional page zoom; see docs/ui.md).
 *
 * Always a sibling of the chip's open action, never nested in it. `reveal`
 * "group" fades the × in while the chip (a `group` ancestor) is hovered or
 * holds focus; the fade sits on a wrapper that is promoted from the start
 * (`will-change`), as the tab strip's is. `wrapperClassName` positions it.
 */
export function ChipRemove({
  label,
  onRemove,
  size = "md",
  reveal = "always",
  className,
  wrapperClassName,
}: {
  /** The accessible name, e.g. `Remove <chip>`. */
  label: string;
  onRemove: () => void;
  size?: keyof typeof ICON;
  reveal?: "always" | "group";
  className?: string;
  wrapperClassName?: string;
}) {
  const button = (
    <button
      type="button"
      aria-label={label}
      onClick={(e) => {
        e.stopPropagation();
        onRemove();
      }}
      className={cn(
        "flex shrink-0 cursor-pointer items-center justify-center rounded-md p-0.5 text-muted-foreground outline-none transition-colors hover:bg-sidebar-item-active hover:text-foreground focus-visible:ring-[3px] focus-visible:ring-ring/50",
        className,
      )}
    >
      <X size={ICON[size]} weight="bold" />
    </button>
  );
  if (reveal === "always") return button;
  return (
    <span
      className={cn(
        "opacity-0 transition-opacity will-change-[opacity] group-focus-within:opacity-100 group-hover:opacity-100",
        wrapperClassName,
      )}
    >
      {button}
    </span>
  );
}

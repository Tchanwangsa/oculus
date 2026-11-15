import type { ReactElement } from "react";
import type { Icon } from "@phosphor-icons/react";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";

/** What an unresolved reference says on hover (after T3's context chips). */
export const UNRESOLVED_TEXT = "This context is no longer available.";

/** A reference whose file is gone, at rest: muted and struck, never removed. */
export const unresolvedClass = "rounded-sm px-0.5 py-px text-muted-foreground line-through decoration-muted-foreground/60";

/** A reference's glyph with a stroke through it, muted. */
export function StruckIcon({ icon: Glyph }: { icon: Icon }) {
  return (
    <span aria-hidden className="relative mr-[0.35em] inline-block align-[-0.18em] text-muted-foreground">
      <Glyph size="1.15em" className="block" />
      <span className="absolute top-1/2 left-1/2 h-px w-[1.3em] -translate-x-1/2 -translate-y-1/2 -rotate-45 bg-current" />
    </span>
  );
}

/** The tooltip an unresolved reference wears in place of its preview. */
export function Unresolved({ children }: { children: ReactElement }) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>{children}</TooltipTrigger>
      <TooltipContent>{UNRESOLVED_TEXT}</TooltipContent>
    </Tooltip>
  );
}

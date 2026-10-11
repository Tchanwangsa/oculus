import { cn } from "@/lib/utils";
import { navigateActive } from "@/lib/shell/tabRouters";
import { useActivePath } from "@/stores/shell/tabStore";
import type { Icon as PhosphorIcon } from "@phosphor-icons/react";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";

/** A rail button's box; the search button shares it. */
export const railButton =
  "relative flex h-9 w-9 shrink-0 items-center justify-center rounded-lg transition-colors";
export const railIdle =
  "text-muted-foreground hover:bg-sidebar-item-hover hover:text-foreground";

interface RailItemProps {
  to: string;
  icon: PhosphorIcon;
  label: string;
  /** Other paths this item also lights on, tested like `to` — for a section
   *  whose tabs aren't under one prefix (Tasks: `/projects` + `/tasks`). */
  match?: readonly string[];
  /** Pinned to the top-right corner: a job spinner or a new-files dot. */
  indicator?: React.ReactNode;
}

/**
 * An icon in the sidebar rail, named by its tooltip. Not a `NavLink`: the
 * sidebar is outside every tab's router, so it navigates the front tab via
 * `navigateActive`; ⌘-click opens a new tab off `data-tab-href`
 * (`app/src/lib/shell/newTabClicks.ts`).
 */
export default function RailItem({ to, icon: Icon, label, match, indicator }: RailItemProps) {
  // Prefix match on `${to}/`, so Home (`/`) tests "//" and lights only on `/`.
  const here = useActivePath().split("?")[0];
  const lightsOn = (path: string) => here === path || here.startsWith(`${path}/`);
  const isActive = lightsOn(to) || (match?.some(lightsOn) ?? false);

  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button
          type="button"
          aria-label={label}
          /* Not an href (outside every router); read by the ⌘-click net. */
          data-tab-href={to}
          onClick={() => navigateActive(to)}
          className={cn(
            railButton,
            isActive ? "bg-sidebar-item-active text-foreground" : railIdle,
          )}
        >
          <Icon size={18} />
          {indicator && (
            <span aria-hidden className="absolute top-1 right-1 flex">
              {indicator}
            </span>
          )}
        </button>
      </TooltipTrigger>
      <TooltipContent side="right">{label}</TooltipContent>
    </Tooltip>
  );
}

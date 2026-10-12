import { NavLink } from "react-router-dom";
import { WarningCircle } from "@phosphor-icons/react";

import { LevelMark } from "@/components/studio/LevelMark";
import { ThemeThumb } from "@/components/studio/themes/ThemeThumb";
import { Skeleton } from "@/components/ui/skeleton";
import type { ThemesState } from "@/hooks/studio/useThemes";
import { themeHref } from "@/lib/studio/routes";
import type { Tokens } from "@/lib/studio/themeDraft";
import type { ThemeDefaults } from "@/lib/studio/themes";
import { cn } from "@/lib/utils";

/**
 * The themes in the browsed library, each as a thumbnail with its name, a
 * "Default" mark on the theme new videos here take, and where it comes from
 * (in a subject's library). The open theme's thumbnail follows its unsaved
 * draft.
 */
export function ThemeColumn({
  state,
  defaults,
  subject,
  open,
  draft,
}: {
  state: ThemesState;
  defaults: ThemeDefaults | null;
  /** The library's course code; null is the global library. */
  subject: string | null;
  /** The theme on the stage. */
  open: string | null;
  /** Its draft, when it has unsaved edits. */
  draft: Tokens | null;
}) {
  return (
    <nav aria-label="Themes" className="min-h-0 w-[208px] shrink-0 overflow-y-auto border-r border-border-subtle px-2 pb-4 pt-2">
      {state.loading && state.themes.length === 0 ? (
        <div className="space-y-2 p-1">
          {Array.from({ length: 4 }, (_, i) => (
            <Skeleton key={i} className="aspect-video w-full rounded-md" />
          ))}
        </div>
      ) : state.error ? (
        <p data-selectable className="px-2 py-1.5 text-[12px] leading-snug text-destructive">
          {state.error}
        </p>
      ) : (
        <div className="flex flex-col gap-1">
          {state.themes.map((t) => (
            <NavLink
              key={t.name}
              to={themeHref(t.name)}
              title={t.problem ?? t.name}
              className={({ isActive }) =>
                cn(
                  "flex flex-col gap-1.5 rounded-xl p-2 transition-colors",
                  isActive ? "bg-accent" : "hover:bg-accent/60",
                )
              }
            >
              <ThemeThumb tokens={t.name === open && draft ? draft : t.tokens} />
              <span className="flex min-w-0 items-center gap-1.5 px-0.5 text-[12.5px] font-medium text-foreground">
                <span className="min-w-0 flex-1 truncate">{t.name}</span>
                {t.problem != null && (
                  <WarningCircle size={13} weight="fill" aria-label="Does not load" className="shrink-0 text-destructive" />
                )}
                {defaults?.effective === t.name && (
                  <span className="shrink-0 text-[10.5px] font-medium text-brand">Default</span>
                )}
                <LevelMark level={t.level} shadows={t.shadows != null} inSubject={subject != null} />
              </span>
            </NavLink>
          ))}
        </div>
      )}
    </nav>
  );
}

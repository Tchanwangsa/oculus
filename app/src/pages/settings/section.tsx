import { Info } from "@phosphor-icons/react";

import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";

/** The DOM id of the `Section` titled `title`, which Settings search scrolls
 *  to (`app/src/lib/settingsSearch.ts`). */
export function settingsSectionId(title: string): string {
  const slug = title.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "");
  return `settings-${slug}`;
}

/** A titled settings group — header + description on top, rows beneath.
 *  Flat, no cards: the section header does the separating. */
export function Section({
  title,
  description,
  children,
}: {
  title: string;
  description?: string;
  children: React.ReactNode;
}) {
  return (
    <section id={settingsSectionId(title)}>
      <h2 className="text-[15px] font-semibold text-foreground">{title}</h2>
      {description && (
        <p className="text-xs text-muted-foreground mt-0.5">{description}</p>
      )}
      <div className="mt-3">{children}</div>
    </section>
  );
}

/** One label/value line. `hint` is a caveat on the value, shown as a tooltip
 *  rather than a second line. */
export function StatRow({
  label,
  value,
  hint,
}: {
  label: string;
  value: string;
  hint?: string;
}) {
  return (
    <div className="flex items-center justify-between gap-4 py-2">
      <span className="flex items-center gap-1 text-xs text-muted-foreground">
        {label}
        {hint ? (
          <Tooltip>
            <TooltipTrigger asChild>
              <span aria-label={hint}>
                <Info size={12} weight="bold" />
              </span>
            </TooltipTrigger>
            <TooltipContent>{hint}</TooltipContent>
          </Tooltip>
        ) : null}
      </span>
      <span className="text-xs text-foreground tabular-nums">{value}</span>
    </div>
  );
}

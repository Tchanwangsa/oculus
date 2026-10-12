import type { KeyboardEvent, ReactNode } from "react";
import { cn } from "@/lib/utils";

/**
 * One choice or service inside a step's body — an agent, a parse engine, an
 * embedding key. Props:
 * - `icon`: a logo or glyph node (`ProviderMark`, a Phosphor icon at 20px),
 *   drawn in a rounded tile that turns indigo when `selected`.
 * - `title`: the name; `chip`: a status slot beside it, usually a `Badge`
 *   ("Signed in", "Not installed").
 * - `detail`: one muted line under the title (version, what it does).
 * - `actions`: buttons on the right; their clicks never select the card.
 * - `children`: full-width content under the row (a key field, a terminal
 *   line).
 * - `selected`: the chosen or ready card — indigo border and a soft ring.
 * - `onSelect`: makes the whole card a radio (click, Enter or Space); put
 *   selectable cards in a `role="radiogroup"`. Without it the card is static.
 */
export function OptionCard({
  icon,
  title,
  chip,
  detail,
  actions,
  children,
  selected = false,
  onSelect,
  className,
}: {
  icon: ReactNode;
  title: ReactNode;
  chip?: ReactNode;
  detail?: ReactNode;
  actions?: ReactNode;
  children?: ReactNode;
  selected?: boolean;
  onSelect?: () => void;
  className?: string;
}) {
  // A div, not a button: `actions` holds buttons, and WebKit drops clicks on
  // a button nested in a button (docs/ui.md).
  const select = onSelect
    ? {
        role: "radio" as const,
        "aria-checked": selected,
        tabIndex: 0,
        onClick: onSelect,
        onKeyDown: (e: KeyboardEvent) => {
          if (e.target !== e.currentTarget || (e.key !== "Enter" && e.key !== " ")) return;
          e.preventDefault();
          onSelect();
        },
      }
    : {};

  return (
    <div
      {...select}
      data-selected={selected || undefined}
      className={cn(
        "group/option rounded-xl border bg-card p-4 transition-[border-color,background-color,box-shadow] outline-none",
        selected
          ? "border-brand/50 bg-brand-muted/30 shadow-xs ring-3 ring-brand/10"
          : "border-border",
        onSelect &&
          "cursor-pointer focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50",
        onSelect && !selected && "hover:border-input hover:bg-surface/40",
        className,
      )}
    >
      <div className="flex items-center gap-3.5">
        <span
          className={cn(
            "flex size-10 shrink-0 items-center justify-center rounded-lg border transition-[color,background-color,border-color] [&>svg]:size-5",
            selected
              ? "border-brand/20 bg-brand-muted text-brand"
              : "border-border-subtle bg-surface text-foreground",
          )}
        >
          {icon}
        </span>
        <div className="min-w-0 flex-1">
          <div className="flex min-w-0 items-center gap-2">
            <span className="truncate text-sm font-medium text-foreground">{title}</span>
            {chip}
          </div>
          {detail && <div className="mt-0.5 text-xs text-muted-foreground">{detail}</div>}
        </div>
        {actions && (
          // Keeps an action's click from also selecting the card.
          <div className="flex shrink-0 items-center gap-2" onClick={(e) => e.stopPropagation()}>
            {actions}
          </div>
        )}
      </div>
      {children && <div className="mt-3.5">{children}</div>}
    </div>
  );
}

import type { ReactNode } from "react";
import { CaretLeft, CaretRight } from "@phosphor-icons/react";
import type { SettingsPage } from "./pages";

/**
 * One row of the settings list: an icon, what the setting is, its value
 * muted at the right and a chevron. A click turns the popover into the
 * row's page (`page`), in place.
 */
export function SettingsRow({
  page,
  icon,
  label,
  value,
  title,
  onOpen,
}: {
  page: Exclude<SettingsPage, "root">;
  icon: ReactNode;
  label: string;
  /** The muted value at the right (a reasoning level, a mode, a usage figure). */
  value?: string | null;
  title?: string;
  onOpen: (page: Exclude<SettingsPage, "root">) => void;
}) {
  return (
    <button
      type="button"
      data-settings-row={page}
      title={title}
      onClick={() => onOpen(page)}
      className="flex h-8 w-full cursor-default items-center gap-2 rounded-md px-2 text-left text-xs text-foreground outline-none transition-colors hover:bg-accent focus-visible:bg-accent"
    >
      <span className="flex size-4 shrink-0 items-center justify-center text-muted-foreground">{icon}</span>
      <span className="min-w-0 flex-1 truncate">{label}</span>
      {value && <span className="shrink-0 tabular-nums text-muted-foreground">{value}</span>}
      <CaretRight className="size-3 shrink-0 text-muted-foreground" aria-hidden />
    </button>
  );
}

/** A page's header: a back control named for the page, and anything the
 *  page keeps at the right (a mode page's warning). */
export function PageHeader({ title, onBack, trailing }: { title: string; onBack: () => void; trailing?: ReactNode }) {
  return (
    <div className="flex h-9 shrink-0 items-center gap-2 border-b border-border-subtle pl-1 pr-3">
      <button
        type="button"
        data-page-back
        aria-label={`${title}, back to settings`}
        title="Back"
        onClick={onBack}
        className="flex h-7 min-w-0 cursor-default items-center gap-1 rounded-md pl-1 pr-2 text-xs font-medium text-foreground outline-none transition-colors hover:bg-accent focus-visible:bg-accent"
      >
        <CaretLeft className="size-3.5 shrink-0 text-muted-foreground" aria-hidden />
        <span className="truncate">{title}</span>
      </button>
      {trailing && <div className="ml-auto flex shrink-0 items-center">{trailing}</div>}
    </div>
  );
}

import type { ReactNode } from "react";

/** A settings row: label and description on the left, the control on the
 *  right, and an optional muted note under the description. */
export function AppearanceRow({
  label,
  description,
  note,
  children,
}: {
  label: string;
  description: string;
  note?: string | null;
  children: ReactNode;
}) {
  return (
    <div className="flex items-start justify-between gap-6 py-2">
      <div className="min-w-0">
        <p className="text-xs text-foreground">{label}</p>
        <p className="text-xs text-muted-foreground mt-0.5">{description}</p>
        {note && <p className="text-xs text-muted-foreground mt-1">{note}</p>}
      </div>
      <div className="flex shrink-0 items-center gap-2">{children}</div>
    </div>
  );
}

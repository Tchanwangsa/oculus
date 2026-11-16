import type { ReactNode } from "react";

/** A label/control line — the writable twin of `StatRow`. */
export function SettingRow({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: string;
  children: ReactNode;
}) {
  return (
    <div className="settings-row flex items-start justify-between gap-6 py-2.5">
      <div className="min-w-0">
        <div className="text-xs text-foreground">{label}</div>
        {hint && (
          <p className="mt-0.5 text-[11px] text-muted-foreground">{hint}</p>
        )}
      </div>
      <div className="settings-controls shrink-0">{children}</div>
    </div>
  );
}

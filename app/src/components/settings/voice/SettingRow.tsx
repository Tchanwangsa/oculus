import type { KeyboardEvent, ReactNode } from "react";

/** Enter commits a text field by blurring it, so blur is the one save path. */
export const blurOnEnter = (e: KeyboardEvent<HTMLInputElement>) => {
  if (e.key === "Enter") e.currentTarget.blur();
};

/** A label and hint on the left, the control on the right, and the row's
 *  own error or note beneath. */
export function SettingRow({ label, hint, error, note, children }: {
  label: string;
  hint: string;
  error?: string | null;
  note?: string | null;
  children: ReactNode;
}) {
  return (
    <div className="py-2">
      <div className="settings-row flex items-center justify-between gap-4">
        <div className="min-w-0">
          <p className="text-xs text-foreground">{label}</p>
          <p className="text-[11px] text-muted-foreground">{hint}</p>
        </div>
        <div className="settings-controls shrink-0">{children}</div>
      </div>
      {error ? (
        <p className="mt-2 text-[11px] leading-relaxed text-destructive" data-selectable>
          {error}
        </p>
      ) : note ? (
        <p className="mt-2 text-[11px] leading-relaxed text-muted-foreground">{note}</p>
      ) : null}
    </div>
  );
}

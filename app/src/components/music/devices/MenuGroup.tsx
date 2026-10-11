import type { ReactNode } from "react";

/** One labelled group in the device menu, an action beside its label. */
export function MenuGroup({
  label,
  action,
  children,
}: {
  label: string;
  action?: ReactNode;
  children: ReactNode;
}) {
  return (
    <div>
      <div className="flex h-6 items-center justify-between pl-2">
        <span className="truncate text-[11px] font-medium text-muted-foreground">{label}</span>
        {action}
      </div>
      {children}
    </div>
  );
}

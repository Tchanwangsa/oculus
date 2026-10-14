import type { ReactNode } from "react";

import { Button } from "@/components/ui/button";

/**
 * A pill in the workspace header. Disabled with a `reason`, it shows the
 * reason as a tooltip from a wrapper, since a disabled button gets no
 * pointer events.
 */
export function HeaderAction({
  onClick,
  reason,
  variant = "secondary",
  children,
}: {
  onClick: () => void;
  /** Why it can't be used now; null enables it. */
  reason: string | null;
  variant?: "default" | "secondary";
  children: ReactNode;
}) {
  return (
    <span title={reason ?? undefined} className="shrink-0">
      <Button size="xs" variant={variant} disabled={reason != null} onClick={onClick}>
        {children}
      </Button>
    </span>
  );
}

import type { ReactNode } from 'react';

/** A run of style bar controls, divided from the one before it by a hairline. */
export function StyleGroup({ children }: { children: ReactNode }) {
  return <div className="flex shrink-0 items-center gap-1 not-first:ml-1.5 not-first:border-l not-first:border-border-subtle not-first:pl-1.5">{children}</div>;
}

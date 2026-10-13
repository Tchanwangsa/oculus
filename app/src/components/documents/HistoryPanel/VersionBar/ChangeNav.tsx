import type { RefObject } from "react";
import { CaretDown, CaretUp } from "@phosphor-icons/react";

import type { VersionViewHandle, VersionViewInfo } from "@/components/documents/VersionView";
import { Button } from "@/components/ui/button";

import { changesLabel } from "./labels";

/** Steps through the version's changes, which the view scrolls to and flashes. */
export function ChangeNav({
  info,
  handle,
}: {
  info: VersionViewInfo | null;
  handle: RefObject<VersionViewHandle | null>;
}) {
  const none = !info || info.hunks === 0;
  return (
    <div className="flex min-w-0 items-center gap-1">
      <Button
        variant="ghost"
        size="icon-xs"
        aria-label="Previous change"
        title="Previous change"
        disabled={none}
        className="text-muted-foreground"
        onClick={() => handle.current?.prev()}
      >
        <CaretUp size={13} />
      </Button>
      <Button
        variant="ghost"
        size="icon-xs"
        aria-label="Next change"
        title="Next change"
        disabled={none}
        className="text-muted-foreground"
        onClick={() => handle.current?.next()}
      >
        <CaretDown size={13} />
      </Button>
      <span aria-live="polite" className="ml-0.5 min-w-0 truncate text-[12px] whitespace-nowrap text-foreground tabular-nums">
        {info ? changesLabel(info) : " "}
      </span>
    </div>
  );
}

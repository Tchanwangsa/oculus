import { memo, type RefObject } from "react";
import { FindBar } from "@/components/ui/FindBar";
import { useDomFind } from "@/hooks/useDomFind";
import { useFindTarget } from "@/lib/find";

/**
 * DOM find over `rootRef`, as a floating bar at the positioned parent's
 * top-right — outside every scroller, so it stays put. With `page`, it is
 * that pane's page-level target (`lib/find.ts`), the last resort for ⌘F.
 */
export const PageFind = memo(function PageFind({
  rootRef,
  page,
}: {
  rootRef: RefObject<HTMLElement | null>;
  page?: number;
}) {
  const find = useDomFind(rootRef);
  useFindTarget(rootRef, { open: find.openFind, step: find.step }, page, true);
  if (!find.open) return null;
  return <FindBar {...find.bar} variant="floating" placeholder="Find on page" />;
});

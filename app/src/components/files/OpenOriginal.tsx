import { ArrowSquareOut } from "@phosphor-icons/react";
import { IconAction } from "@/components/markdown/embed/IconAction";
import { isEdUrl } from "@/lib/ed/links";
import { openLinkOut, type LinkOut } from "@/lib/files/canvasLinks";

/** The one "open the original" icon: a synced page, thread, lesson or video
 *  card opens where it came from, in an in-app tab (⌘-click: the system
 *  browser). An Ed URL is labelled "Open in Ed"; anything else says Canvas. */
export function OpenOriginal({
  href,
  label,
  size,
}: {
  href: LinkOut;
  label?: string;
  size?: "icon-xs" | "icon-sm";
}) {
  const text = label ?? (typeof href === "string" && isEdUrl(href) ? "Open in Ed" : "Open in Canvas");
  return (
    <IconAction label={text} size={size} onClick={(e) => void openLinkOut(href, e.metaKey)}>
      <ArrowSquareOut size={14} />
    </IconAction>
  );
}

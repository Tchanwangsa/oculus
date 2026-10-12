import { ArrowSquareOut, CircleNotch, X } from "@phosphor-icons/react";
import { IconAction } from "@/components/markdown/embed/IconAction";
import { openLinkOut, type LinkOut } from "@/lib/files/canvasLinks";

/** The small Open in Canvas icon a card keeps beside its main action;
 *  ⌘-click opens the system browser. */
export function OpenInCanvasIcon({ href }: { href: LinkOut }) {
  return (
    <IconAction label="Open in Canvas" size="icon-sm" onClick={(e) => void openLinkOut(href, e.metaKey)}>
      <ArrowSquareOut size={14} />
    </IconAction>
  );
}

/** An in-flight download: the percent (a spinner before the first byte, or
 *  `label` for a phase without one) and Cancel. */
export function DownloadProgress({
  percent,
  label,
  hint,
  onCancel,
}: {
  percent: number | null;
  label?: string;
  hint?: string;
  onCancel: () => void;
}) {
  return (
    <>
      <span
        className="flex h-8 min-w-10 items-center justify-end px-1 text-xs tabular-nums text-brand"
        title={hint}
      >
        {label ?? (percent == null ? <CircleNotch size={14} className="animate-spin" /> : `${percent}%`)}
      </span>
      <IconAction label="Cancel download" size="icon-sm" onClick={onCancel}>
        <X size={14} />
      </IconAction>
    </>
  );
}

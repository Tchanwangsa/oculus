import { cn } from "@/lib/utils";

/** The dashed "drop here" card over a whole drop target (`useFileDrop`). The
 *  parent must be `relative`; the overlay never takes pointer events. */
export function DropOverlay({
  show,
  label,
  compact,
}: {
  show: boolean;
  label: string;
  /** For a narrow dock: a tighter inset and smaller label. */
  compact?: boolean;
}) {
  return (
    <div
      aria-hidden
      className={cn(
        "pointer-events-none absolute z-10 flex items-center justify-center rounded-xl border-2 border-dashed border-brand bg-brand/5 transition-opacity duration-150",
        compact ? "inset-1.5" : "inset-3",
        show ? "opacity-100" : "opacity-0",
      )}
    >
      <span
        className={cn(
          "rounded-full bg-card font-medium text-brand shadow-sm",
          compact ? "px-2.5 py-1 text-[11.5px]" : "px-3 py-1.5 text-[13px]",
        )}
      >
        {label}
      </span>
    </div>
  );
}

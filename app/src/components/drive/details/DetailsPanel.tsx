import { Info, X } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import type { DriveItem } from "@/lib/drive/items";
import { fmtSize } from "@/lib/format/format";
import { ItemDetails } from "./ItemDetails";

/**
 * The details panel beside the entries, opened and closed from the toolbar
 * (never by selecting): one picked entry's details, how many are picked and
 * what their files weigh, or, with none, where to start.
 */
export function DetailsPanel({ picked, onClose }: { picked: DriveItem[]; onClose: () => void }) {
  return (
    <aside className="flex w-64 shrink-0 flex-col gap-3 overflow-y-auto border-l border-border-subtle px-4 pt-2 pb-4">
      <div className="flex h-7 shrink-0 items-center justify-between">
        <span className="text-[12px] font-medium text-muted-foreground">
          {picked.length > 1 ? `${picked.length} items selected` : "Details"}
        </span>
        <Button size="icon-xs" variant="ghost" aria-label="Hide details" title="Hide details" onClick={onClose}>
          <X aria-hidden />
        </Button>
      </div>
      {picked.length === 1 ? (
        <ItemDetails item={picked[0]} />
      ) : picked.length > 1 ? (
        <Several items={picked} />
      ) : (
        <div className="flex flex-col items-center gap-2 px-2 pt-10 text-center text-[12px] text-muted-foreground">
          <Info size={22} className="opacity-70" aria-hidden />
          Select a file or folder to see its details.
        </div>
      )}
    </aside>
  );
}

function Several({ items }: { items: DriveItem[] }) {
  const files = items.filter((i) => i.type === "file");
  const folders = items.filter((i) => i.type === "folder").length;
  const size = files.reduce((sum, i) => sum + (i.type === "file" ? i.entry.size : 0), 0);
  const parts = [
    files.length ? `${files.length} ${files.length === 1 ? "file" : "files"}` : null,
    folders ? `${folders} ${folders === 1 ? "folder" : "folders"}` : null,
  ].filter(Boolean);
  return (
    <dl className="grid grid-cols-[auto_minmax(0,1fr)] gap-x-3 gap-y-1.5 text-[12px]">
      <dt className="text-muted-foreground">Picked</dt>
      <dd className="min-w-0 truncate text-foreground tabular-nums">{parts.join(", ") || `${items.length} items`}</dd>
      {files.length > 0 && (
        <>
          <dt className="text-muted-foreground">Size</dt>
          <dd className="min-w-0 truncate text-foreground tabular-nums">{fmtSize(size)}</dd>
        </>
      )}
    </dl>
  );
}

import type { ReactNode } from "react";
import { ArrowSquareUpRight, Copy } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { FileRecency } from "@/components/files/FileRecency";
import { useFileParseState } from "@/components/files/ParseState";
import type { DbFile } from "@/lib/db";
import { KIND_LABEL, kindOf, SOURCE_LABEL, sourceOf } from "@/lib/drive/filters";
import { itemZone, type DriveItem } from "@/lib/drive/items";
import { fmtFullStamp, fmtSize } from "@/lib/format/format";
import { useDriveView } from "../DriveContext";
import { recordingDay } from "../content/meta";
import { Thumbnail } from "../thumbs/Thumbnail";

/**
 * The one selected file's picture and facts beside the entries: what it is,
 * where it came from, its parse and search state, size, when it changed and
 * when it was last opened. A synced file offers Copy to my files. Only drawn
 * where the pane is wide enough; on a narrow one the menu and the row say
 * the same.
 */
export function DetailsPanel({ item }: { item: DriveItem }) {
  const view = useDriveView();
  const synced = itemZone(item) === "synced" && item.type === "file";
  return (
    <aside className="flex w-64 shrink-0 flex-col gap-3 overflow-y-auto border-l border-border-subtle px-4 py-4">
      <Thumbnail item={item} className="aspect-[4/3] w-full rounded-lg border border-border-subtle" />
      <h4 className="text-[13px] leading-snug font-semibold break-words text-foreground">{item.name}</h4>
      <dl className="grid grid-cols-[auto_minmax(0,1fr)] gap-x-3 gap-y-1.5 text-[12px]">
        {item.type === "file" ? <FileFacts item={item} row={item.row} /> : <RecordingFacts item={item} />}
      </dl>
      <div className="flex flex-wrap gap-1.5">
        <Button size="xs" variant="outline" onClick={() => view.open(item)}>
          <ArrowSquareUpRight aria-hidden />Open
        </Button>
        {synced && (
          <Button size="xs" variant="outline" onClick={() => view.copyToMine([item])}>
            <Copy aria-hidden />Copy to my files
          </Button>
        )}
      </div>
    </aside>
  );
}

function Fact({ label, children }: { label: string; children: ReactNode }) {
  return (
    <>
      <dt className="text-muted-foreground">{label}</dt>
      <dd className="min-w-0 truncate text-foreground tabular-nums">{children}</dd>
    </>
  );
}

function FileFacts({ item, row }: { item: Extract<DriveItem, { type: "file" }>; row: DbFile | undefined }) {
  const parse = useFileParseState(row ?? null);
  const { entry } = item;
  return (
    <>
      <Fact label="Kind">{KIND_LABEL[kindOf(entry.name, row?.category)]}</Fact>
      <Fact label="From">{SOURCE_LABEL[sourceOf(entry.zone, row?.category)]}</Fact>
      {parse && <Fact label="Parse">{parse.title}</Fact>}
      {parse && <Fact label="Search">{row?.embed_status === "done" ? "Indexed" : "Not indexed"}</Fact>}
      <Fact label="Size">{fmtSize(entry.size)}</Fact>
      {entry.modified != null && <Fact label={entry.zone === "own" ? "Modified" : "Synced"}>{fmtFullStamp(entry.modified)}</Fact>}
      {row && <Fact label="Opened"><FileRecency file={row} /></Fact>}
    </>
  );
}

function RecordingFacts({ item }: { item: DriveItem }) {
  if (item.type !== "recording") return null;
  return (
    <>
      <Fact label="Kind">Recording</Fact>
      <Fact label="Date">{recordingDay(item.entry.date)}</Fact>
    </>
  );
}

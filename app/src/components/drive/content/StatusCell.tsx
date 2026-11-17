import { ParseStateBadge, useFileParseState } from "@/components/files/ParseState";
import type { DbFile } from "@/lib/db";
import { KIND_LABEL, kindOf } from "@/lib/drive/filters";
import type { DriveItem } from "@/lib/drive/items";
import { plural } from "./meta";

const capital = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);

/** A list row's Kind or Status cell: a parsed file's state (the row's
 *  `ParseStateBadge`, which acts on a click, and its word), else what the
 *  entry is. */
export function StatusCell({ item }: { item: DriveItem }) {
  if (item.type === "folder") {
    return <Muted>{item.subject ? "Subject" : plural(item.entry.childCount, "item")}</Muted>;
  }
  if (item.type === "recording") return <Muted>Recording</Muted>;
  const kind = KIND_LABEL[kindOf(item.entry.name, item.row?.category)];
  return item.row ? <ParseCell row={item.row} kind={kind} /> : <Muted>{kind}</Muted>;
}

function ParseCell({ row, kind }: { row: DbFile; kind: string }) {
  const state = useFileParseState(row);
  if (!state) return <Muted>{kind}</Muted>;
  return (
    // Its own click: the badge retries or opens the file, never selects.
    <span className="flex min-w-0 items-center gap-1.5" onClick={(e) => e.stopPropagation()}>
      <ParseStateBadge file={row} />
      <span className="truncate text-[11.5px] text-muted-foreground">{capital(state.label)}</span>
    </span>
  );
}

function Muted({ children }: { children: string }) {
  return <span className="truncate text-[11.5px] text-muted-foreground">{children}</span>;
}

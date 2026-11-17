import type { DriveItem } from "@/lib/drive/items";
import { cn } from "@/lib/utils";
import { useDriveView } from "../DriveContext";
import { EntryMenu } from "../menu/EntryMenu";
import { Thumbnail } from "../thumbs/Thumbnail";
import { CARD, cardState } from "./cardStyle";
import { EntryName } from "./EntryName";
import { ItemIcon } from "./ItemIcon";
import { itemProps } from "./itemProps";
import { entryMeta } from "./meta";

/** A file or recording as a card: its picture over its glyph and name, and
 *  under them when or where it is. The rounded clip stays promoted so the
 *  picture's layer never re-snaps the glyph (`docs/ui.md`). */
export function FileCard({ item }: { item: DriveItem }) {
  const view = useDriveView();
  const meta = entryMeta(item, view);
  return (
    <EntryMenu item={item}>
      <div
        {...itemProps(item, view)}
        title={item.name}
        className={cn(CARD, "flex flex-col overflow-hidden will-change-transform", cardState(view.selected.has(item.key), null))}
      >
        <Thumbnail item={item} className="aspect-[4/3] w-full border-b border-border-subtle" />
        <div className="min-w-0 px-2.5 py-2">
          <div className="flex min-w-0 items-center gap-2 text-[12px] text-foreground">
            <ItemIcon item={item} size={14} />
            <EntryName item={item} />
          </div>
          {meta && <div className="mt-0.5 truncate pl-[22px] text-[11px] text-muted-foreground tabular-nums">{meta}</div>}
        </div>
      </div>
    </EntryMenu>
  );
}

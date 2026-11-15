import { useRef, type KeyboardEvent } from "react";
import { Image } from "@phosphor-icons/react";
import { ChipRemove } from "@/components/ui/ChipRemove";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { fmtAgo } from "@/lib/format/format";
import { useAttachmentNames } from "@/stores/chat/attachmentNames";
import { MAX_STASH, stashSnippet, type StashEntry } from "@/stores/chat/stashStore";
import { StashBadge } from "./StashBadge";

/** Said inside the menu, never as a toast, after a stash pushed one out. */
export const STASH_EVICTED = `The stash holds ${MAX_STASH} prompts; the oldest was removed to make room.`;

/**
 * "Stashed prompts": the badge above the composer and its popover, newest
 * first, each row restoring on click (Enter) with its own Delete. Arrows move
 * between rows. Draws nothing while the stash is empty.
 */
export function StashMenu({
  entries,
  evicted,
  open,
  onOpenChange,
  onRestore,
  onDelete,
}: {
  entries: StashEntry[];
  evicted: boolean;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onRestore: (entry: StashEntry) => void;
  onDelete: (entry: StashEntry) => void;
}) {
  const listRef = useRef<HTMLUListElement>(null);
  /** A restore puts the caret in the box; the popover must not take it back
   *  to the badge as it closes. */
  const restored = useRef(false);
  /** An attachment chip in a row's words reads as its name. */
  const names = useAttachmentNames((s) => s.names);
  if (!entries.length) return null;

  function move(e: KeyboardEvent<HTMLUListElement>) {
    if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
    const rows = Array.from(listRef.current?.querySelectorAll<HTMLButtonElement>("[data-stash-restore]") ?? []);
    if (!rows.length) return;
    e.preventDefault();
    const at = rows.indexOf(document.activeElement as HTMLButtonElement);
    const next = e.key === "ArrowDown" ? (at + 1) % rows.length : (at - 1 + rows.length) % rows.length;
    rows[at < 0 ? 0 : next]?.focus();
  }

  return (
    <Popover open={open} onOpenChange={onOpenChange}>
      <PopoverTrigger asChild>
        <StashBadge count={entries.length} />
      </PopoverTrigger>
      <PopoverContent
        align="end"
        side="top"
        sideOffset={6}
        className="flex max-h-[min(var(--radix-popover-content-available-height),26rem)] w-80 flex-col overflow-y-auto p-1"
        onCloseAutoFocus={(e) => {
          if (restored.current) e.preventDefault();
          restored.current = false;
        }}
      >
        <div className="px-2 pt-1.5 pb-1 text-[11px] text-muted-foreground">Stashed prompts</div>
        {evicted && (
          <p className="mx-1 mb-1 rounded-md bg-muted px-2 py-1.5 text-[11px] leading-snug text-muted-foreground">{STASH_EVICTED}</p>
        )}
        <ul ref={listRef} aria-label="Stashed prompts" onKeyDown={move}>
          {entries.map((entry) => {
            const snippet = stashSnippet(entry, 90, (path) => names[path]);
            return (
              <li key={entry.id} className="flex items-center gap-2 rounded-md px-2 py-1.5 hover:bg-accent focus-within:bg-accent">
                <button
                  type="button"
                  data-stash-restore
                  aria-label={`Restore stashed prompt: ${snippet}`}
                  onClick={() => {
                    restored.current = true;
                    onRestore(entry);
                  }}
                  className="min-w-0 flex-1 cursor-pointer truncate text-left text-xs text-foreground outline-none"
                >
                  {snippet}
                </button>
                {entry.pictures.length > 0 && (
                  <span className="flex shrink-0 items-center gap-0.5 text-[11px] text-muted-foreground tabular-nums">
                    <Image size={11} aria-hidden />
                    {entry.pictures.length}
                  </span>
                )}
                <span className="shrink-0 text-[11px] text-muted-foreground tabular-nums">{fmtAgo(entry.createdAt)}</span>
                <ChipRemove label="Delete stashed prompt" onRemove={() => onDelete(entry)} />
              </li>
            );
          })}
        </ul>
      </PopoverContent>
    </Popover>
  );
}

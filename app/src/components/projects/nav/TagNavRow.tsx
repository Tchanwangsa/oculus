import { useState } from "react";
import { Link } from "react-router-dom";
import { DotsThree, Trash } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { ConfirmDialog } from "@/components/ui/ConfirmDialog";
import { Input } from "@/components/ui/input";
import {
  SIDE_NAV_FOLDS,
  SIDE_NAV_ROW,
  SIDE_NAV_ROW_ACTIVE,
  SIDE_NAV_ROW_IDLE,
} from "@/components/ui/layout/SideNav";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import {
  MAX_TAG_NAME,
  deleteTaskTag,
  updateTaskTag,
  type DbTaskTag,
  type TagColor,
} from "@/lib/planning/projects";
import { TagColorSwatches } from "../tags/TagColorSwatches";
import { TagDot } from "../tags/TagDot";

/** A message for a failed write, shown inline (there are no toasts). */
function message(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

/**
 * A tag in the nav column: a link that selects it, its task count, and a `…`
 * that appears on hover (or focus, or while its menu is open) for rename,
 * colour and delete. The `…` sits over the link, not inside it, so no button
 * is nested in an anchor.
 */
export function TagNavRow({
  tag,
  to,
  active,
  count,
  onDeleted,
}: {
  tag: DbTaskTag;
  /** Where a click goes: the tag selected, or cleared if it already is. */
  to: string;
  active: boolean;
  count: number;
  /** After the tag is gone, so a page showing it can step off it. */
  onDeleted: () => void;
}) {
  const [menu, setMenu] = useState(false);
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);
  const [name, setName] = useState(tag.name);
  const [error, setError] = useState<string | null>(null);

  const rename = async () => {
    if (name.trim() === "" || name.trim() === tag.name) {
      setName(tag.name);
      setError(null);
      return;
    }
    try {
      await updateTaskTag(tag.id, { name });
      setError(null);
    } catch (e) {
      setError(message(e));
    }
  };

  const recolor = (color: TagColor) => {
    updateTaskTag(tag.id, { color }).catch((e) => setError(message(e)));
  };

  const remove = async () => {
    setBusy(true);
    try {
      await deleteTaskTag(tag.id);
      setConfirming(false);
      onDeleted();
    } catch (e) {
      setConfirming(false);
      setMenu(true);
      setError(message(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="group/tag relative">
      <Link
        to={to}
        aria-current={active ? "page" : undefined}
        className={cn(SIDE_NAV_ROW, active ? SIDE_NAV_ROW_ACTIVE : SIDE_NAV_ROW_IDLE)}
      >
        <span className="flex size-4 shrink-0 items-center justify-center">
          <TagDot color={tag.color} />
        </span>
        <span className={cn("min-w-0 flex-1 truncate", SIDE_NAV_FOLDS)}>{tag.name}</span>
        <span
          className={cn(
            "w-6 shrink-0 text-right text-[11px] tabular-nums text-muted-foreground/70",
            "group-hover/tag:opacity-0 group-has-[button:focus-visible]/tag:opacity-0",
            menu && "opacity-0",
            SIDE_NAV_FOLDS,
          )}
        >
          {count}
        </span>
      </Link>

      <Popover
        open={menu}
        onOpenChange={(open) => {
          setMenu(open);
          if (!open) {
            setName(tag.name);
            setError(null);
          }
        }}
      >
        <PopoverTrigger asChild>
          <button
            type="button"
            aria-label={`Edit tag ${tag.name}`}
            className={cn(
              "absolute top-1/2 right-1.5 flex size-5 -translate-y-1/2 cursor-pointer items-center justify-center rounded text-muted-foreground outline-none",
              "opacity-0 group-hover/tag:opacity-100 focus-visible:opacity-100 data-[state=open]:opacity-100",
              "hover:bg-sidebar-item-active hover:text-foreground focus-visible:ring-[3px] focus-visible:ring-ring/50",
            )}
          >
            <DotsThree size={16} weight="bold" />
          </button>
        </PopoverTrigger>
        <PopoverContent align="start" className="w-60 p-3">
          <Input
            autoFocus
            value={name}
            maxLength={MAX_TAG_NAME}
            aria-label="Tag name"
            aria-invalid={error != null}
            onChange={(e) => {
              setName(e.target.value);
              setError(null);
            }}
            onBlur={() => void rename()}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                void rename();
              }
            }}
            className="h-8 rounded-lg text-[13px]"
          />
          {error && <p className="mt-1.5 px-0.5 text-[11px] text-destructive">{error}</p>}
          <TagColorSwatches value={tag.color} onChange={recolor} className="mt-2.5 -ml-1 flex-wrap" />
          <Button
            variant="ghost"
            size="sm"
            onClick={() => {
              setMenu(false);
              setConfirming(true);
            }}
            className="mt-2 -mb-1 w-full justify-start text-destructive hover:bg-destructive/10 hover:text-destructive"
          >
            <Trash size={13} />
            Delete tag
          </Button>
        </PopoverContent>
      </Popover>

      <ConfirmDialog
        open={confirming}
        onCancel={() => setConfirming(false)}
        onConfirm={() => void remove()}
        busy={busy}
        title={`Delete “${tag.name}”?`}
        description="This removes the tag from every task that has it. The tasks themselves stay."
        confirmLabel="Delete tag"
        busyLabel="Deleting…"
      />
    </div>
  );
}

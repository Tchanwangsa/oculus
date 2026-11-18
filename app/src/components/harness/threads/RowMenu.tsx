import type { ReactNode } from "react";
import { DropdownMenu } from "radix-ui";
import { ArrowSquareOut, DotsThree, Trash } from "@phosphor-icons/react";
import { shortcut } from "@/lib/platform";
import { cn } from "@/lib/utils";

const ITEM =
  "flex cursor-pointer items-center gap-2.5 rounded-lg px-2 py-1.5 text-[12.5px] outline-none data-highlighted:bg-accent";

function Item({
  icon,
  children,
  hint,
  destructive,
  onSelect,
}: {
  icon: ReactNode;
  children: ReactNode;
  hint?: string;
  destructive?: boolean;
  onSelect: () => void;
}) {
  return (
    <DropdownMenu.Item
      onSelect={onSelect}
      className={cn(ITEM, destructive && "text-destructive data-highlighted:bg-destructive/10")}
    >
      <span className={cn("flex size-4 items-center justify-center", destructive ? "text-destructive" : "text-muted-foreground")}>
        {icon}
      </span>
      <span className="flex-1">{children}</span>
      {hint && <span className="text-[11px] text-muted-foreground">{hint}</span>}
    </DropdownMenu.Item>
  );
}

/** A thread row's ⋯ menu: open it in a new tab, or delete it (which arms the
 *  row's inline confirm rather than deleting at once). */
export function RowMenu({
  title,
  onNewTab,
  onDelete,
  className,
}: {
  title: string;
  onNewTab: () => void;
  onDelete: () => void;
  className?: string;
}) {
  return (
    <DropdownMenu.Root>
      <DropdownMenu.Trigger asChild>
        <button
          type="button"
          aria-label={`Actions for ${title}`}
          title="More"
          className={cn(
            "flex h-[22px] w-6 items-center justify-center rounded-[5px] text-muted-foreground outline-none transition-colors hover:bg-sidebar-item-active hover:text-foreground data-[state=open]:text-foreground",
            className,
          )}
        >
          <DotsThree size={14} weight="bold" />
        </button>
      </DropdownMenu.Trigger>
      <DropdownMenu.Portal>
        <DropdownMenu.Content
          side="bottom"
          align="end"
          sideOffset={4}
          collisionPadding={12}
          className="z-50 min-w-48 rounded-xl border border-border-subtle bg-popover p-1 text-popover-foreground shadow-lg outline-none"
          // Delete arms a confirm that takes focus; the trigger must not take it back.
          onCloseAutoFocus={(e) => e.preventDefault()}
        >
          <Item icon={<ArrowSquareOut size={14} />} hint={`${shortcut("mod")} click`} onSelect={onNewTab}>
            Open in new tab
          </Item>
          <DropdownMenu.Separator className="mx-1 my-1 h-px bg-border-subtle" />
          <Item icon={<Trash size={14} />} destructive onSelect={onDelete}>
            Delete
          </Item>
        </DropdownMenu.Content>
      </DropdownMenu.Portal>
    </DropdownMenu.Root>
  );
}

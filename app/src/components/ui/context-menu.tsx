import * as React from "react";
import { ContextMenu as ContextMenuPrimitive } from "radix-ui";
import { CaretRight } from "@phosphor-icons/react";

import { cn } from "@/lib/utils";

/**
 * shadcn's context menu on Radix, drawn as the app's other menus are
 * (`RowMenu`, `PageMenu`): a `rounded-xl` popover with `rounded-lg` rows, a
 * muted 16px icon column and a muted shortcut hint on the right. Items take
 * their icon as `icon`; `destructive` tints a row red.
 */

const CONTENT =
  "z-50 min-w-48 overflow-hidden rounded-xl border border-border-subtle bg-popover p-1 text-popover-foreground shadow-lg outline-none";

const ITEM =
  "flex cursor-pointer select-none items-center gap-2.5 rounded-lg px-2 py-1.5 text-[12.5px] outline-none data-disabled:pointer-events-none data-disabled:opacity-50 data-highlighted:bg-accent data-[state=open]:bg-accent";

function ContextMenu(props: React.ComponentProps<typeof ContextMenuPrimitive.Root>) {
  return <ContextMenuPrimitive.Root data-slot="context-menu" {...props} />;
}

function ContextMenuTrigger(props: React.ComponentProps<typeof ContextMenuPrimitive.Trigger>) {
  return <ContextMenuPrimitive.Trigger data-slot="context-menu-trigger" {...props} />;
}

function ContextMenuContent({ className, ...props }: React.ComponentProps<typeof ContextMenuPrimitive.Content>) {
  return (
    <ContextMenuPrimitive.Portal>
      <ContextMenuPrimitive.Content
        data-slot="context-menu-content"
        collisionPadding={12}
        className={cn(CONTENT, className)}
        {...props}
      />
    </ContextMenuPrimitive.Portal>
  );
}

function ItemIcon({ icon, destructive }: { icon?: React.ReactNode; destructive?: boolean }) {
  return (
    <span className={cn("flex size-4 shrink-0 items-center justify-center", destructive ? "text-destructive" : "text-muted-foreground")}>
      {icon}
    </span>
  );
}

function ContextMenuItem({
  className,
  icon,
  hint,
  destructive,
  children,
  ...props
}: React.ComponentProps<typeof ContextMenuPrimitive.Item> & {
  icon?: React.ReactNode;
  /** A shortcut, right-aligned and muted. */
  hint?: string;
  destructive?: boolean;
}) {
  return (
    <ContextMenuPrimitive.Item
      data-slot="context-menu-item"
      className={cn(ITEM, destructive && "text-destructive data-highlighted:bg-destructive/10", className)}
      {...props}
    >
      <ItemIcon icon={icon} destructive={destructive} />
      <span className="min-w-0 flex-1 truncate">{children}</span>
      {hint && <span className="shrink-0 text-[11px] text-muted-foreground">{hint}</span>}
    </ContextMenuPrimitive.Item>
  );
}

function ContextMenuSub(props: React.ComponentProps<typeof ContextMenuPrimitive.Sub>) {
  return <ContextMenuPrimitive.Sub data-slot="context-menu-sub" {...props} />;
}

function ContextMenuSubTrigger({
  className,
  icon,
  children,
  ...props
}: React.ComponentProps<typeof ContextMenuPrimitive.SubTrigger> & { icon?: React.ReactNode }) {
  return (
    <ContextMenuPrimitive.SubTrigger data-slot="context-menu-sub-trigger" className={cn(ITEM, className)} {...props}>
      <ItemIcon icon={icon} />
      <span className="min-w-0 flex-1 truncate">{children}</span>
      <CaretRight size={11} className="shrink-0 text-muted-foreground" />
    </ContextMenuPrimitive.SubTrigger>
  );
}

function ContextMenuSubContent({ className, ...props }: React.ComponentProps<typeof ContextMenuPrimitive.SubContent>) {
  return (
    <ContextMenuPrimitive.Portal>
      <ContextMenuPrimitive.SubContent
        data-slot="context-menu-sub-content"
        collisionPadding={12}
        className={cn(CONTENT, className)}
        {...props}
      />
    </ContextMenuPrimitive.Portal>
  );
}

function ContextMenuSeparator({ className, ...props }: React.ComponentProps<typeof ContextMenuPrimitive.Separator>) {
  return (
    <ContextMenuPrimitive.Separator
      data-slot="context-menu-separator"
      className={cn("mx-1 my-1 h-px bg-border-subtle", className)}
      {...props}
    />
  );
}

/** A muted line under a group of items, saying why they are off. */
function ContextMenuNote({ icon, children }: { icon?: React.ReactNode; children: React.ReactNode }) {
  return (
    <div className="flex items-center gap-2.5 px-2 pt-1 pb-1.5 text-[11.5px] text-muted-foreground">
      <ItemIcon icon={icon} />
      <span className="min-w-0 flex-1">{children}</span>
    </div>
  );
}

export {
  ContextMenu,
  ContextMenuTrigger,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSub,
  ContextMenuSubTrigger,
  ContextMenuSubContent,
  ContextMenuSeparator,
  ContextMenuNote,
};

import type { KeyboardEvent, PointerEvent, ReactNode, Ref } from "react";
import { DotsSixVertical, Eye, EyeSlash, type Icon } from "@phosphor-icons/react";
import { cn } from "@/lib/utils";

/** One row of the property list: handle, icon, name, eye. Every row is the
 *  same height, which `useStripReorder` relies on. */
export function PropertyRow({
  icon: RowIcon,
  label,
  position,
  visible,
  locked = false,
  onToggle,
  drag,
}: {
  icon: Icon;
  label: string;
  /** "2 of 5", for the handle's accessible name. */
  position?: string;
  visible: boolean;
  /** Name: shown, greyed, not a control. */
  locked?: boolean;
  onToggle?: () => void;
  /** Present on a row that can be dragged; absent leaves the handle's space empty. */
  drag?: {
    rowRef: Ref<HTMLLIElement>;
    grabbed: boolean;
    dragging: boolean;
    style?: React.CSSProperties;
    handleRef: Ref<HTMLButtonElement>;
    onPointerDown: (e: PointerEvent<HTMLElement>) => void;
    onClickCapture: (e: React.MouseEvent) => void;
    onKeyDown: (e: KeyboardEvent<HTMLButtonElement>) => void;
  };
}) {
  return (
    <li
      ref={drag?.rowRef}
      onPointerDown={drag?.onPointerDown}
      // Swallow the click a drag's release raises, before the eye hears it.
      onClickCapture={drag?.onClickCapture}
      style={drag?.style}
      className={cn(
        "flex h-8 items-center gap-1 rounded-md bg-popover pr-1 pl-0.5 will-change-transform",
        drag?.grabbed
          ? "relative z-10 shadow-md"
          : drag?.dragging && "transition-transform duration-200 ease-out",
      )}
    >
      {drag ? (
        <button
          ref={drag.handleRef}
          type="button"
          aria-label={`${label}, position ${position}: the arrow keys move it`}
          onKeyDown={drag.onKeyDown}
          className={cn(
            "flex size-6 shrink-0 cursor-grab items-center justify-center rounded-md text-muted-foreground/60 outline-none",
            "hover:text-foreground focus-visible:ring-[3px] focus-visible:ring-ring/50",
            drag.grabbed && "cursor-grabbing",
          )}
        >
          <DotsSixVertical size={14} weight="bold" />
        </button>
      ) : (
        <span aria-hidden className="size-6 shrink-0" />
      )}
      <RowIcon size={14} className="shrink-0 text-muted-foreground" />
      <span className="min-w-0 flex-1 truncate text-[12.5px] text-foreground">{label}</span>
      {locked ? (
        <span
          title="Name is always shown"
          className="flex size-6 shrink-0 items-center justify-center text-muted-foreground/40"
        >
          <Eye size={14} />
        </span>
      ) : (
        <button
          type="button"
          aria-label={visible ? `Hide ${label}` : `Show ${label}`}
          onClick={onToggle}
          className="flex size-6 shrink-0 cursor-pointer items-center justify-center rounded-md text-muted-foreground outline-none hover:bg-accent hover:text-foreground focus-visible:ring-[3px] focus-visible:ring-ring/50"
        >
          {visible ? <Eye size={14} /> : <EyeSlash size={14} />}
        </button>
      )}
    </li>
  );
}

export function PropertySectionLabel({
  children,
  action,
}: {
  children: ReactNode;
  action?: { label: string; onClick: () => void };
}) {
  return (
    <div className="flex h-7 items-center justify-between px-1">
      <span className="text-[11px] font-medium text-muted-foreground">{children}</span>
      {action && (
        <button
          type="button"
          onClick={action.onClick}
          className="cursor-pointer rounded text-[11px] text-muted-foreground outline-none hover:text-foreground focus-visible:ring-[3px] focus-visible:ring-ring/50"
        >
          {action.label}
        </button>
      )}
    </div>
  );
}

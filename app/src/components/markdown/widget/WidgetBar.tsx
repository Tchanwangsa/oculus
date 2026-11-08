import type { ReactNode } from "react";
import { ArrowsOutSimple, ChatCircle, CircleNotch, Code } from "@phosphor-icons/react";
import { IconAction } from "@/components/markdown/embed/IconAction";
import { Pill } from "@/components/markdown/openui/components/Frame";
import { cn } from "@/lib/utils";

/** The card a widget sits in, Frame's look: the title bar, then the body. */
export function WidgetCard({ md, children }: { md: string; children: ReactNode }) {
  return (
    <div data-md={md} className="group/widget my-3 overflow-hidden rounded-xl border border-border bg-card">
      {children}
    </div>
  );
}

/** Title, and once the widget runs, Source and Expand; `save` (shown on
 *  hover or focus) and `status` (a request running) sit before them.
 *  `building`: the reply is still writing it, so the title shimmers and the
 *  bar says so, as a streaming proposal card does. */
export function WidgetBar({
  title,
  building = false,
  save,
  status,
  source,
  onSource,
  onExpand,
}: {
  title: string;
  building?: boolean;
  save?: ReactNode;
  status?: ReactNode;
  /** Whether the source is showing; absent while building. */
  source?: boolean;
  onSource?: () => void;
  onExpand?: () => void;
}) {
  return (
    <div className="flex h-9 min-w-0 select-none items-center gap-1 border-b border-border pr-1 pl-3">
      <span
        className={cn("min-w-0 flex-1 truncate font-display text-[13px] font-semibold text-foreground", building && "text-shimmer")}
        title={title}
      >
        {title}
      </span>
      {building && <span className="shrink-0 pr-2 text-xs text-muted-foreground">Building…</span>}
      {save && (
        <div className="opacity-0 transition-opacity group-hover/widget:opacity-100 focus-within:opacity-100">{save}</div>
      )}
      {status}
      {onSource && (
        <IconAction label={source ? "Hide source" : "Source"} onClick={onSource}>
          <Code size={13} weight={source ? "bold" : "regular"} />
        </IconAction>
      )}
      {onExpand && (
        <IconAction label="Expand" onClick={onExpand}>
          <ArrowsOutSimple size={13} />
        </IconAction>
      )}
    </div>
  );
}

/** While the reply streams: never the source, never a frame. */
export function Building({ title }: { title: string }) {
  return (
    <div className="flex items-center gap-2 px-3 py-2.5 text-xs">
      <CircleNotch size={13} className="shrink-0 animate-spin text-muted-foreground" />
      <span className="min-w-0 truncate text-shimmer">Building {title}…</span>
    </div>
  );
}

/** A streaming widget before its first preview draws: a block where it
 *  will be, shimmering as OpenUI's unfinished parts do. */
export function Sketch() {
  return (
    <div className="p-3" aria-hidden="true">
      <div className="block-shimmer h-24 rounded-lg" />
    </div>
  );
}

/** Before the frame's first height report. */
export function Loading() {
  return (
    <div className="flex h-10 items-center justify-center text-muted-foreground">
      <CircleNotch size={14} className="animate-spin" />
    </div>
  );
}

/** One line in the destructive tone, and Ask to fix where a thread listens. */
export function WidgetError({ message, onAsk }: { message: string; onAsk?: () => void }) {
  return (
    <div className="flex min-w-0 items-center gap-2 border-b border-border px-3 py-1.5">
      <span data-selectable className="min-w-0 flex-1 truncate text-xs text-destructive" title={message}>
        {message}
      </span>
      {onAsk && (
        <Pill onClick={onAsk}>
          <ChatCircle size={13} /> Ask to fix
        </Pill>
      )}
    </div>
  );
}

import { memo, useState } from "react";
import { Check, Copy } from "@phosphor-icons/react";
import type { Icon } from "@phosphor-icons/react";
import { CompactMd } from "@/components/markdown/MdComponents";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { fmtClockSecs } from "@/lib/lectures/media";
import { cn, copyText } from "@/lib/utils";

// Memoised: a committed reply never changes, and re-parsing it per streamed
// token is what makes the thread jitter.
export const Assistant = memo(function Assistant({ text }: { text: string }) {
  return <CompactMd text={text} className="px-2 text-[13px] leading-relaxed" />;
});

/** One action under a message: an icon with a tooltip. */
export function Action({
  label,
  icon: Icon,
  onClick,
}: {
  label: string;
  icon: Icon;
  onClick: () => void;
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button
          type="button"
          aria-label={label}
          onClick={onClick}
          className="cursor-pointer rounded-full p-1.5 text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
        >
          <Icon size={14} />
        </button>
      </TooltipTrigger>
      <TooltipContent>{label}</TooltipContent>
    </Tooltip>
  );
}

/** The icon becomes a tick on copy (no toasts in this app). */
export function CopyAction({ text }: { text: string }) {
  const [done, setDone] = useState(false);
  return (
    <Action
      label={done ? "Copied" : "Copy"}
      icon={done ? Check : Copy}
      onClick={() => {
        void copyText(text).then((ok) => {
          if (!ok) return;
          setDone(true);
          setTimeout(() => setDone(false), 1200);
        });
      }}
    />
  );
}

/** The row under a message: its time, then its actions. Its height is always
 *  taken and only the contents fade in on hover, so the thread never jumps.
 *  `will-change` keeps it on its own layer: WebKit otherwise drops and re-adds
 *  the layer around each fade, re-snapping the icons half a pixel at a
 *  fractional page zoom. */
export function MessageActions({
  when,
  at,
  side,
  children,
}: {
  when?: string;
  /** The playhead second a dock question was asked at. */
  at?: number | null;
  side: "left" | "right";
  children: React.ReactNode;
}) {
  return (
    <div
      // Kept out of a copied selection (`selectionMarkdown`), and not
      // selectable at all: a drag across it highlights the icons, which reads
      // as them bolding and unbolding.
      data-copy-skip
      className={cn(
        "-mt-0.5 flex select-none h-8 items-center gap-1 text-[11px] text-muted-foreground opacity-0 transition-opacity will-change-[opacity] focus-within:opacity-100 group-hover/msg:opacity-100",
        side === "right" ? "justify-end" : "pl-1",
      )}
    >
      {when && <span className="px-1.5 tabular-nums">{when}</span>}
      {at != null && (
        <span className="-ml-1 pr-1.5 tabular-nums" title="The moment this message carried">
          at {fmtClockSecs(at)}
        </span>
      )}
      {children}
    </div>
  );
}
